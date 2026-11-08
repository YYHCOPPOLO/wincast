// Package notes owns the pure notes decisions: search filtering and the
// active-note choice after a deletion.
package notes

import (
	"strings"

	"github.com/YYHCOPPOLO/wincast/go/internal/storage"
)

// Filter matches every whitespace term against the title and file name. An
// empty query lists metadata only, capped at 200 rows.
func Filter(summaries []storage.NoteSummary, query string) []storage.NoteSummary {
	terms := strings.Fields(strings.ToLower(query))
	hits := make([]storage.NoteSummary, 0, len(summaries))
	if len(terms) == 0 {
		if len(summaries) > 200 {
			return summaries[:200]
		}
		return summaries
	}
	for _, summary := range summaries {
		haystack := strings.ToLower(summary.Title + " " + summary.ID)
		match := true
		for _, term := range terms {
			if !strings.Contains(haystack, term) {
				match = false
				break
			}
		}
		if match {
			hits = append(hits, summary)
			if len(hits) == 200 {
				break
			}
		}
	}
	return hits
}

// Successor chooses the note to show after deleting id: the next row, else
// the previous, else none.
func Successor(summaries []storage.NoteSummary, id string) (storage.NoteSummary, bool) {
	for index, summary := range summaries {
		if summary.ID != id {
			continue
		}
		if index+1 < len(summaries) {
			return summaries[index+1], true
		}
		if index > 0 {
			return summaries[index-1], true
		}
		return storage.NoteSummary{}, false
	}
	if len(summaries) > 0 {
		return summaries[0], true
	}
	return storage.NoteSummary{}, false
}
