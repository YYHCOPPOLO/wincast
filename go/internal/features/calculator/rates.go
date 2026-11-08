package calculator

import (
	"context"
	"encoding/json"
	"errors"
	"io"
	"log"
	"math"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"

	domaincalc "github.com/YYHCOPPOLO/wincast/go/internal/domain/calculator"
	"github.com/YYHCOPPOLO/wincast/go/internal/storage"
)

const (
	FiatURL      = "https://api.frankfurter.app/latest"
	CoinURL      = "https://api.coingecko.com/api/v3/simple/price?ids=bitcoin,ethereum,solana&vs_currencies=usd"
	refreshAge   = 24 * 60 * 60
	retryAge     = 30 * 60
	fetchTimeout = 8 * time.Second
)

// Rates keeps the last fiat/crypto snapshot. Fetching is a background effect;
// the calculator engine only ever receives an immutable snapshot.
type Rates struct {
	mu          sync.RWMutex
	rates       *domaincalc.Rates
	completedAt int64
	cachePath   string
	client      *http.Client
	onUpdate    func()
}

func (s *Rates) SetOnUpdate(callback func()) {
	s.mu.Lock()
	s.onUpdate = callback
	s.mu.Unlock()
}

type cachedRates struct {
	FetchedAt    int64              `json:"fetched_at"`
	UnitsPerBase map[string]float64 `json:"units_per_base"`
}

// UnmarshalJSON accepts both this build's object map and the legacy cache's
// array of [code, unitsPerBase] pairs.
func (c *cachedRates) UnmarshalJSON(data []byte) error {
	var raw struct {
		FetchedAt int64           `json:"fetched_at"`
		Units     json.RawMessage `json:"units_per_base"`
	}
	if err := json.Unmarshal(data, &raw); err != nil {
		return err
	}
	c.FetchedAt = raw.FetchedAt
	c.UnitsPerBase = make(map[string]float64)
	if len(raw.Units) == 0 {
		return nil
	}
	var asMap map[string]float64
	if err := json.Unmarshal(raw.Units, &asMap); err == nil {
		c.UnitsPerBase = asMap
		return nil
	}
	var asArray [][]json.RawMessage
	if err := json.Unmarshal(raw.Units, &asArray); err != nil {
		return err
	}
	for _, pair := range asArray {
		if len(pair) != 2 {
			continue
		}
		var code string
		var rate float64
		if json.Unmarshal(pair[0], &code) == nil && json.Unmarshal(pair[1], &rate) == nil {
			c.UnitsPerBase[strings.ToUpper(code)] = rate
		}
	}
	return nil
}

func NewRates(cachePath string) *Rates {
	store := &Rates{cachePath: cachePath, client: &http.Client{Timeout: fetchTimeout}}
	if cached, ok := loadRatesCache(cachePath); ok {
		store.rates = cached
		if domaincalc.PricesCoins(cached) {
			store.completedAt = cached.FetchedAt
		}
	}
	return store
}

func (s *Rates) Snapshot() *domaincalc.Rates {
	s.mu.RLock()
	defer s.mu.RUnlock()
	return s.rates
}

// Run refreshes the snapshot until ctx is canceled. A fresh cache waits for
// its age; a failed fetch retries after 30 minutes.
func (s *Rates) Run(ctx context.Context) {
	for {
		now := time.Now().Unix()
		age := int64(0)
		if s.completedAt > 0 {
			age = now - s.completedAt
			if age < 0 {
				age = 0
			}
		} else {
			age = refreshAge
		}
		if age < refreshAge {
			if !sleepContext(ctx, time.Duration(refreshAge-age)*time.Second) {
				return
			}
			continue
		}
		rates, complete, err := s.fetch(ctx, now)
		if err != nil {
			if ctx.Err() != nil {
				return
			}
			log.Printf("calculator rates fetch failed: %v", err)
			if !sleepContext(ctx, retryAge*time.Second) {
				return
			}
			continue
		}
		s.mu.Lock()
		s.rates = rates
		if complete {
			s.completedAt = rates.FetchedAt
		}
		onUpdate := s.onUpdate
		s.mu.Unlock()
		if onUpdate != nil {
			onUpdate()
		}
		if complete {
			if err := saveRatesCache(s.cachePath, rates); err != nil {
				log.Printf("calculator rates cache save failed: %v", err)
			}
			if !sleepContext(ctx, refreshAge*time.Second) {
				return
			}
		} else if !sleepContext(ctx, retryAge*time.Second) {
			return
		}
	}
}

func (s *Rates) fetch(ctx context.Context, now int64) (*domaincalc.Rates, bool, error) {
	fiatBody, err := s.get(ctx, FiatURL)
	if err != nil {
		return nil, false, err
	}
	fiat, err := parseFrankfurter(fiatBody)
	if err != nil {
		return nil, false, err
	}
	var crypto []domaincalc.RatePair
	if cryptoBody, err := s.get(ctx, CoinURL); err == nil {
		crypto = parseCoinGecko(cryptoBody)
	}
	rates := domaincalc.MergeFeeds(fiat, crypto)
	rates.FetchedAt = now
	return &rates, domaincalc.PricesCoins(&rates), nil
}

func (s *Rates) get(ctx context.Context, url string) ([]byte, error) {
	request, err := http.NewRequestWithContext(ctx, http.MethodGet, url, nil)
	if err != nil {
		return nil, err
	}
	request.Header.Set("Accept", "application/json")
	request.Header.Set("User-Agent", "Wincast/0.2 (Windows)")
	response, err := s.client.Do(request)
	if err != nil {
		return nil, err
	}
	defer response.Body.Close()
	if response.StatusCode != http.StatusOK {
		return nil, errors.New("unexpected status: " + response.Status)
	}
	return io.ReadAll(io.LimitReader(response.Body, 1<<20))
}

func parseFrankfurter(data []byte) ([]domaincalc.RatePair, error) {
	var payload struct {
		Base  string             `json:"base"`
		Rates map[string]float64 `json:"rates"`
	}
	if err := json.Unmarshal(data, &payload); err != nil {
		return nil, err
	}
	base := strings.ToUpper(payload.Base)
	if len(base) != 3 || len(payload.Rates) == 0 {
		return nil, errors.New("invalid fiat rates")
	}
	pairs := make([]domaincalc.RatePair, 0, len(payload.Rates)+1)
	for code, rate := range payload.Rates {
		if usableRate(rate) {
			pairs = append(pairs, domaincalc.RatePair{Code: strings.ToUpper(code), Rate: rate})
		}
	}
	if len(pairs) == 0 {
		return nil, errors.New("empty fiat rates")
	}
	pairs = append(pairs, domaincalc.RatePair{Code: base, Rate: 1})
	usd := 0.0
	for _, pair := range pairs {
		if pair.Code == "USD" {
			usd = pair.Rate
			break
		}
	}
	if !usableRate(usd) {
		return nil, errors.New("fiat rates have no USD base")
	}
	for i := range pairs {
		if pairs[i].Code == "USD" {
			pairs[i].Rate = 1
		} else {
			pairs[i].Rate /= usd
		}
	}
	return pairs, nil
}

func parseCoinGecko(data []byte) []domaincalc.RatePair {
	var payload map[string]struct {
		USD float64 `json:"usd"`
	}
	if err := json.Unmarshal(data, &payload); err != nil {
		return nil
	}
	mapping := []struct{ id, code string }{
		{"bitcoin", "BTC"}, {"ethereum", "ETH"}, {"solana", "SOL"},
	}
	pairs := make([]domaincalc.RatePair, 0, len(mapping))
	for _, item := range mapping {
		price := payload[item.id].USD
		if !usableRate(price) || !usableRate(1/price) {
			continue
		}
		pairs = append(pairs, domaincalc.RatePair{Code: item.code, Rate: 1 / price})
	}
	return pairs
}

func usableRate(rate float64) bool {
	return rate > 0 && !isInfOrNaN(rate)
}

func isInfOrNaN(value float64) bool {
	return math.IsNaN(value) || math.IsInf(value, 0)
}

func loadRatesCache(path string) (*domaincalc.Rates, bool) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, false
	}
	var cached cachedRates
	if err := json.Unmarshal(data, &cached); err != nil || cached.UnitsPerBase == nil {
		return nil, false
	}
	return &domaincalc.Rates{FetchedAt: cached.FetchedAt, UnitsPerBase: cached.UnitsPerBase}, true
}

func saveRatesCache(path string, rates *domaincalc.Rates) error {
	if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
		return err
	}
	data, err := json.MarshalIndent(cachedRates{FetchedAt: rates.FetchedAt, UnitsPerBase: rates.UnitsPerBase}, "", "  ")
	if err != nil {
		return err
	}
	data = append(data, '\n')
	return storage.WriteFileAtomic(path, data, 0o600)
}

func sleepContext(ctx context.Context, duration time.Duration) bool {
	timer := time.NewTimer(duration)
	defer timer.Stop()
	select {
	case <-ctx.Done():
		return false
	case <-timer.C:
		return true
	}
}
