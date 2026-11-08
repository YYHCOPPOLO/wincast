// Package windowlayout holds the pure DIP geometry used by window management.
package windowlayout

import (
	"cmp"
	"math"
	"slices"

	"github.com/YYHCOPPOLO/wincast/go/internal/domain/windowcommand"
)

const (
	stepFraction   = 0.05
	almostMax      = 0.9
	reasonable     = 0.6
	reasonableMaxW = 1025
	reasonableMaxH = 900
	oneThird       = 1.0 / 3.0
	twoThirds      = 2.0 / 3.0
)

type DipRect struct {
	X, Y, W, H float64
}

type Screen struct {
	ID      int
	Frame   DipRect
	Visible DipRect
}

type Axis uint8

const (
	AxisMin Axis = iota
	AxisCenter
	AxisMax
)

type Anchor struct {
	Horizontal Axis
	Vertical   Axis
}

var (
	TopLeading = Anchor{Horizontal: AxisMin, Vertical: AxisMin}
	Centered   = Anchor{Horizontal: AxisCenter, Vertical: AxisCenter}
)

func (a Anchor) Place(w, h float64, slot DipRect) DipRect {
	return DipRect{X: origin(a.Horizontal, slot.X, slot.W, w), Y: origin(a.Vertical, slot.Y, slot.H, h), W: w, H: h}
}

func origin(axis Axis, min, length, size float64) float64 {
	switch axis {
	case AxisCenter:
		return min + (length-size)/2
	case AxisMax:
		return min + length - size
	}
	return min
}

type Placement struct {
	Frame    DipRect
	ScreenID int
	Anchor   Anchor
	Resizes  bool
}

type LayoutInput struct {
	Visible DipRect
	Window  DipRect
	Gap     float64
	Step    int
}

type LayoutQuery struct {
	Command  windowcommand.ID
	Window   DipRect
	Screens  []Screen
	Gap      float64
	Step     int
	Restore  *DipRect
	LastTile windowcommand.ID
}

func PlacementFor(id windowcommand.ID, input LayoutInput) (DipRect, bool) {
	screen := Screen{ID: 0, Frame: input.Visible, Visible: input.Visible}
	placement, ok := Resolve(LayoutQuery{
		Command: id, Window: input.Window, Screens: []Screen{screen}, Gap: input.Gap, Step: input.Step,
	})
	if !ok {
		return DipRect{}, false
	}
	return placement.Frame, true
}

func Resolve(input LayoutQuery) (Placement, bool) {
	kind := input.Command.Kind()
	if kind != windowcommand.KindGeometry && kind != windowcommand.KindRestore {
		return Placement{}, false
	}
	if len(input.Screens) == 0 {
		return Placement{}, false
	}
	if kind == windowcommand.KindRestore {
		return restorePlacement(input)
	}
	host, ok := screenContaining(input.Window, input.Screens)
	if !ok || host.Visible.W <= 0 || host.Visible.H <= 0 {
		return Placement{}, false
	}
	gap := SanitizedGap(input.Gap, host.Visible)
	if input.Command == windowcommand.NextDisplay || input.Command == windowcommand.PreviousDisplay {
		return displayPlacement(input, host, gap)
	}
	step := 0
	if input.Command.CyclesOnRepeat() {
		step = normalizedStep(input.Step)
	}
	if fractions, ok := tileFractions(input.Command, step); ok {
		return Placement{
			Frame:    tile(host.Visible, fractions.X0, fractions.X1, fractions.Y0, fractions.Y1, gap),
			ScreenID: host.ID, Anchor: fractions.Anchor, Resizes: true,
		}, true
	}
	canvas := Canvas(host.Visible, gap)
	if canvas.W <= 0 || canvas.H <= 0 {
		return Placement{}, false
	}
	current := input.Window
	switch input.Command {
	case windowcommand.Maximize:
		return Placement{Frame: canvas, ScreenID: host.ID, Anchor: TopLeading, Resizes: true}, true
	case windowcommand.AlmostMaximize:
		return Placement{
			Frame:    Rounded(Centered.Place(canvas.W*almostMax, canvas.H*almostMax, canvas)),
			ScreenID: host.ID, Anchor: Centered, Resizes: true,
		}, true
	case windowcommand.ReasonableSize:
		w := math.Min(canvas.W*reasonable, reasonableMaxW)
		h := math.Min(canvas.H*reasonable, reasonableMaxH)
		return Placement{Frame: Rounded(Centered.Place(w, h, canvas)), ScreenID: host.ID, Anchor: Centered, Resizes: true}, true
	case windowcommand.MaximizeHeight:
		frame := DipRect{X: current.X, Y: canvas.Y, W: current.W, H: canvas.H}
		return Placement{Frame: Rounded(Clamped(frame, canvas)), ScreenID: host.ID, Anchor: TopLeading, Resizes: true}, true
	case windowcommand.MaximizeWidth:
		frame := DipRect{X: canvas.X, Y: current.Y, W: canvas.W, H: current.H}
		return Placement{Frame: Rounded(Clamped(frame, canvas)), ScreenID: host.ID, Anchor: TopLeading, Resizes: true}, true
	case windowcommand.Center:
		w := math.Min(current.W, canvas.W)
		h := math.Min(current.H, canvas.H)
		return Placement{Frame: Rounded(Centered.Place(w, h, canvas)), ScreenID: host.ID, Anchor: Centered, Resizes: true}, true
	case windowcommand.MakeLarger, windowcommand.MakeSmaller:
		return Placement{
			Frame:    resized(current, canvas, input.Command == windowcommand.MakeLarger),
			ScreenID: host.ID, Anchor: Centered, Resizes: true,
		}, true
	case windowcommand.MoveLeft, windowcommand.MoveRight, windowcommand.MoveUp, windowcommand.MoveDown:
		return Placement{Frame: nudged(current, canvas, input.Command), ScreenID: host.ID, Anchor: TopLeading, Resizes: false}, true
	}
	return Placement{}, false
}

func ScreenContaining(frame DipRect, screens []Screen) (Screen, bool) {
	return screenContaining(frame, screens)
}

func screenContaining(frame DipRect, screens []Screen) (Screen, bool) {
	var best Screen
	bestArea := 0.0
	for _, screen := range screens {
		area := IntersectionArea(screen.Frame, frame)
		if area > bestArea {
			best, bestArea = screen, area
		}
	}
	if bestArea > 0 {
		return best, true
	}
	cx, cy := frame.X+frame.W/2, frame.Y+frame.H/2
	for _, screen := range screens {
		if contains(screen.Frame, cx, cy) {
			return screen, true
		}
	}
	if len(screens) > 0 {
		return screens[0], true
	}
	return Screen{}, false
}

func Ordered(screens []Screen) []Screen {
	out := append([]Screen(nil), screens...)
	slices.SortFunc(out, func(a, b Screen) int {
		if byX := cmp.Compare(a.Frame.X, b.Frame.X); byX != 0 {
			return byX
		}
		return cmp.Compare(a.Frame.Y, b.Frame.Y)
	})
	return out
}

func displayPlacement(input LayoutQuery, host Screen, gap float64) (Placement, bool) {
	ordered := Ordered(input.Screens)
	if len(ordered) <= 1 {
		return Placement{}, false
	}
	index := -1
	for i, screen := range ordered {
		if screen.ID == host.ID {
			index = i
			break
		}
	}
	if index < 0 {
		return Placement{}, false
	}
	offset := 1
	if input.Command == windowcommand.PreviousDisplay {
		offset = -1
	}
	dest := ordered[((index+offset)%len(ordered)+len(ordered))%len(ordered)]
	return Placement{
		Frame: moved(input.Window, host, dest, gap, input.LastTile), ScreenID: dest.ID, Anchor: Centered, Resizes: true,
	}, true
}

func moved(frame DipRect, from, to Screen, gap float64, lastTile windowcommand.ID) DipRect {
	if lastTile != "" {
		if fractions, ok := tileFractions(lastTile, 0); ok {
			return tile(to.Visible, fractions.X0, fractions.X1, fractions.Y0, fractions.Y1, SanitizedGap(gap, to.Visible))
		}
	}
	source, target := from.Visible, to.Visible
	if source.W <= 0 || source.H <= 0 {
		return frame
	}
	relativeX := (frame.X - source.X) / source.W
	relativeY := (frame.Y - source.Y) / source.H
	scaled := DipRect{
		X: target.X + relativeX*target.W,
		Y: target.Y + relativeY*target.H,
		W: math.Min(target.W, frame.W/source.W*target.W),
		H: math.Min(target.H, frame.H/source.H*target.H),
	}
	return Rounded(Clamped(scaled, target))
}

func restorePlacement(input LayoutQuery) (Placement, bool) {
	if input.Restore == nil {
		return Placement{}, false
	}
	restore := *input.Restore
	host, ok := screenContaining(restore, input.Screens)
	if !ok {
		return Placement{}, false
	}
	overlap := intersection(host.Visible, restore)
	stranded := overlap.W < 40 || overlap.H < 40
	frame := restore
	if stranded {
		frame = Rounded(Clamped(Centered.Place(restore.W, restore.H, host.Visible), host.Visible))
	}
	return Placement{Frame: frame, ScreenID: host.ID, Anchor: Centered, Resizes: true}, true
}

type fractions struct {
	X0, X1, Y0, Y1 float64
	Anchor         Anchor
}

func tileFractions(id windowcommand.ID, step int) (fractions, bool) {
	cycle := [3]float64{0.5, oneThird, twoThirds}
	position := cycle[normalizedStep(step)]
	switch id {
	case windowcommand.LeftHalf:
		return fractions{0, position, 0, 1, TopLeading}, true
	case windowcommand.RightHalf:
		return fractions{1 - position, 1, 0, 1, Anchor{AxisMax, AxisMin}}, true
	case windowcommand.TopHalf:
		return fractions{0, 1, 0, position, TopLeading}, true
	case windowcommand.BottomHalf:
		return fractions{0, 1, 1 - position, 1, Anchor{AxisMin, AxisMax}}, true
	case windowcommand.TopLeftQuarter:
		return fractions{0, 0.5, 0, 0.5, TopLeading}, true
	case windowcommand.TopRightQuarter:
		return fractions{0.5, 1, 0, 0.5, Anchor{AxisMax, AxisMin}}, true
	case windowcommand.BottomLeftQuarter:
		return fractions{0, 0.5, 0.5, 1, Anchor{AxisMin, AxisMax}}, true
	case windowcommand.BottomRightQuarter:
		return fractions{0.5, 1, 0.5, 1, Anchor{AxisMax, AxisMax}}, true
	case windowcommand.FirstThreeFourths:
		return fractions{0, 0.75, 0, 1, TopLeading}, true
	case windowcommand.LastThreeFourths:
		return fractions{0.25, 1, 0, 1, Anchor{AxisMax, AxisMin}}, true
	case windowcommand.FirstThird:
		return fractions{0, oneThird, 0, 1, TopLeading}, true
	case windowcommand.CenterThird:
		return fractions{oneThird, twoThirds, 0, 1, Anchor{AxisCenter, AxisMin}}, true
	case windowcommand.LastThird:
		return fractions{twoThirds, 1, 0, 1, Anchor{AxisMax, AxisMin}}, true
	case windowcommand.FirstTwoThirds:
		return fractions{0, twoThirds, 0, 1, TopLeading}, true
	case windowcommand.LastTwoThirds:
		return fractions{oneThird, 1, 0, 1, Anchor{AxisMax, AxisMin}}, true
	case windowcommand.CenterHalf:
		return fractions{0.25, 0.75, 0, 1, Anchor{AxisCenter, AxisMin}}, true
	}
	return fractions{}, false
}

func IsTileCommand(id windowcommand.ID) bool {
	_, ok := tileFractions(id, 0)
	return ok
}

func tile(visible DipRect, x0, x1, y0, y1, gap float64) DipRect {
	left := visible.X + x0*visible.W
	if x0 == 0 {
		left += gap
	} else {
		left += gap / 2
	}
	right := visible.X + x1*visible.W
	if x1 == 1 {
		right -= gap
	} else {
		right -= gap / 2
	}
	top := visible.Y + y0*visible.H
	if y0 == 0 {
		top += gap
	} else {
		top += gap / 2
	}
	bottom := visible.Y + y1*visible.H
	if y1 == 1 {
		bottom -= gap
	} else {
		bottom -= gap / 2
	}
	return Rounded(DipRect{X: left, Y: top, W: math.Max(right-left, 1), H: math.Max(bottom-top, 1)})
}

func Canvas(visible DipRect, gap float64) DipRect {
	return Rounded(inset(visible, gap, gap))
}

func minimumSize(canvas DipRect) (float64, float64) {
	return math.Min(canvas.W, math.Max(canvas.W*0.15, 200)), math.Min(canvas.H, math.Max(canvas.H*0.15, 150))
}

func evenStep(dimension float64) float64 {
	return math.Max(math.Round(dimension*stepFraction/2)*2, 2)
}

func resized(frame, canvas DipRect, larger bool) DipRect {
	direction := -1.0
	if larger {
		direction = 1
	}
	floorW, floorH := minimumSize(canvas)
	width := math.Min(canvas.W, math.Max(frame.W+direction*evenStep(canvas.W), floorW))
	height := math.Min(canvas.H, math.Max(frame.H+direction*evenStep(canvas.H), floorH))
	centred := DipRect{X: frame.X - (width-frame.W)/2, Y: frame.Y - (height-frame.H)/2, W: width, H: height}
	return Rounded(Clamped(centred, canvas))
}

func nudged(frame, canvas DipRect, id windowcommand.ID) DipRect {
	dx := math.Round(canvas.W * stepFraction)
	dy := math.Round(canvas.H * stepFraction)
	moved := frame
	switch id {
	case windowcommand.MoveLeft:
		moved.X -= dx
	case windowcommand.MoveRight:
		moved.X += dx
	case windowcommand.MoveUp:
		moved.Y -= dy
	case windowcommand.MoveDown:
		moved.Y += dy
	}
	return Rounded(Clamped(moved, canvas))
}

func Rounded(rect DipRect) DipRect {
	minX, minY := math.Round(rect.X), math.Round(rect.Y)
	maxX, maxY := math.Round(rect.X+rect.W), math.Round(rect.Y+rect.H)
	return DipRect{X: minX, Y: minY, W: math.Max(maxX-minX, 0), H: math.Max(maxY-minY, 0)}
}

func Clamped(frame, box DipRect) DipRect {
	x := math.Min(math.Max(frame.X, box.X), math.Max(box.X, box.X+box.W-frame.W))
	y := math.Min(math.Max(frame.Y, box.Y), math.Max(box.Y, box.Y+box.H-frame.H))
	return DipRect{X: x, Y: y, W: frame.W, H: frame.H}
}

func SanitizedGap(gap float64, visible DipRect) float64 {
	if math.IsNaN(gap) || math.IsInf(gap, 0) || gap <= 0 || visible.W <= 0 || visible.H <= 0 {
		return 0
	}
	return math.Min(gap, math.Min(visible.W, visible.H)/10)
}

func normalizedStep(step int) int {
	return ((step % 3) + 3) % 3
}

func inset(r DipRect, dx, dy float64) DipRect {
	return DipRect{X: r.X + dx, Y: r.Y + dy, W: math.Max(r.W-dx*2, 0), H: math.Max(r.H-dy*2, 0)}
}

func intersection(a, b DipRect) DipRect {
	x0 := math.Max(a.X, b.X)
	y0 := math.Max(a.Y, b.Y)
	x1 := math.Min(a.X+a.W, b.X+b.W)
	y1 := math.Min(a.Y+a.H, b.Y+b.H)
	return DipRect{X: x0, Y: y0, W: math.Max(x1-x0, 0), H: math.Max(y1-y0, 0)}
}

func IntersectionArea(a, b DipRect) float64 {
	r := intersection(a, b)
	return r.W * r.H
}

func contains(r DipRect, x, y float64) bool {
	return x >= r.X && x < r.X+r.W && y >= r.Y && y < r.Y+r.H
}
