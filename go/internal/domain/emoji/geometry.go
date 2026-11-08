package emoji

// GridSection describes one contiguous category in the displayed search
// results. Rendering and keyboard navigation use the same row boundaries.
type GridSection struct {
	Group string
	Start int
	Count int
}

func Sections(items []Emoji) []GridSection {
	var sections []GridSection
	for index, item := range items {
		last := len(sections) - 1
		if last < 0 || sections[last].Group != item.Group {
			sections = append(sections, GridSection{Group: item.Group, Start: index, Count: 1})
		} else {
			sections[last].Count++
		}
	}
	return sections
}

// VerticalSelection preserves the visual column across category boundaries.
// A shorter final row is visited before moving into the next category.
func VerticalSelection(items []Emoji, selected, direction int) int {
	sections := Sections(items)
	for index, section := range sections {
		if selected < section.Start || selected >= section.Start+section.Count {
			continue
		}
		local := selected - section.Start
		if direction > 0 {
			if next := local + GridColumns; next < section.Count {
				return section.Start + next
			}
			if local/GridColumns < (section.Count-1)/GridColumns {
				return section.Start + section.Count - 1
			}
			if index+1 < len(sections) {
				next := sections[index+1]
				return next.Start + min(local%GridColumns, next.Count-1)
			}
		} else {
			if local >= GridColumns {
				return selected - GridColumns
			}
			if index > 0 {
				previous := sections[index-1]
				lastRow := (previous.Count - 1) / GridColumns * GridColumns
				return previous.Start + min(lastRow+local%GridColumns, previous.Count-1)
			}
		}
		return selected
	}
	return selected
}
