package app

import (
	"log"
	"time"

	domaincalc "github.com/YYHCOPPOLO/wincast/go/internal/domain/calculator"
	"github.com/YYHCOPPOLO/wincast/go/internal/platform/windows"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/palette"
)

func (c *controller) startCalculatorRates() {
	c.calcRates.SetOnUpdate(func() {
		c.panel.Update(func() {
			if c.closing {
				return
			}
			if c.view.Mode == palette.ModeLauncher || c.view.Mode == palette.ModeCalculatorHistory {
				c.view.Filter()
			}
		})
	})
	c.workers.Add(1)
	go func() {
		defer c.workers.Done()
		c.calcRates.Run(c.ctx)
	}()
}

// localNaiveUnix encodes local wall-clock fields as UTC seconds, matching the
// Rust oracle's injected `now` for date grammar.
func localNaiveUnix() int64 {
	now := time.Now()
	return time.Date(now.Year(), now.Month(), now.Day(), now.Hour(), now.Minute(), now.Second(), 0, time.UTC).Unix()
}

func (c *controller) calculate(query string) *domaincalc.Result {
	if c.calcRates == nil {
		return domaincalc.Evaluate(query, localNaiveUnix(), nil, c.calcRegion)
	}
	return domaincalc.Evaluate(query, localNaiveUnix(), c.calcRates.Snapshot(), c.calcRegion)
}

func (c *controller) calculatorHistory(query string) []domaincalc.HistoryEntry {
	if c.calcHistory == nil {
		return nil
	}
	return c.calcHistory.Search(query)
}

func (c *controller) persistCalculatorHistory() {
	if c.calcHistory == nil {
		return
	}
	if err := c.calcHistory.Save(c.calcHistoryPath); err != nil {
		log.Printf("calculator history save failed: %v", err)
		c.view.Status = c.view.Text("计算历史保存失败", "Could not save calculator history")
	}
}

func (c *controller) copyCalculatorResult(result domaincalc.Result) {
	if result.IsError() {
		return
	}
	if err := windows.WriteClipboardText(c.panel.NativeHandle(), result.CopyText, true); err != nil {
		c.view.Status = c.view.Text("复制失败，请重试", "Could not copy; please retry")
		log.Printf("calculator copy failed: %v", err)
		return
	}
	if c.calcHistory != nil {
		c.calcHistory.Record(result.Expression, result.CopyText)
		c.persistCalculatorHistory()
	}
	c.view.Status = c.view.Text("已复制答案", "Answer copied")
	c.hide()
	log.Printf("calculator copied expression=%q", result.Expression)
}

func (c *controller) copyCalculatorHistory(entry domaincalc.HistoryEntry) {
	payload := domaincalc.HistoryCopyPayload(entry.Result)
	if err := windows.WriteClipboardText(c.panel.NativeHandle(), payload, true); err != nil {
		c.view.Status = c.view.Text("复制失败，请重试", "Could not copy; please retry")
		log.Printf("calculator history copy failed: %v", err)
		return
	}
	c.view.Status = c.view.Text("已复制历史结果", "History result copied")
	c.hide()
}

func (c *controller) openCalculatorHistory() {
	if c.closing {
		return
	}
	c.view.SetModeCalculatorHistory()
	if !c.panel.IsVisible() {
		c.target = windows.ForegroundTarget()
	}
	c.showPanel(false)
}
