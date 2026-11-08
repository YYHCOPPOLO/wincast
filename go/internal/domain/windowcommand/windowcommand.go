// Package windowcommand holds the window-management catalog. It performs no
// platform work.
package windowcommand

type ID string

const (
	LeftHalf           ID = "left-half"
	RightHalf          ID = "right-half"
	TopHalf            ID = "top-half"
	BottomHalf         ID = "bottom-half"
	TopLeftQuarter     ID = "top-left-quarter"
	TopRightQuarter    ID = "top-right-quarter"
	BottomLeftQuarter  ID = "bottom-left-quarter"
	BottomRightQuarter ID = "bottom-right-quarter"
	FirstThreeFourths  ID = "first-three-fourths"
	LastThreeFourths   ID = "last-three-fourths"
	FirstThird         ID = "first-third"
	CenterThird        ID = "center-third"
	LastThird          ID = "last-third"
	FirstTwoThirds     ID = "first-two-thirds"
	LastTwoThirds      ID = "last-two-thirds"
	Maximize           ID = "maximize"
	AlmostMaximize     ID = "almost-maximize"
	ReasonableSize     ID = "reasonable-size"
	MaximizeHeight     ID = "maximize-height"
	MaximizeWidth      ID = "maximize-width"
	Center             ID = "center"
	CenterHalf         ID = "center-half"
	MakeLarger         ID = "make-larger"
	MakeSmaller        ID = "make-smaller"
	Restore            ID = "restore"
	MoveLeft           ID = "move-left"
	MoveRight          ID = "move-right"
	MoveUp             ID = "move-up"
	MoveDown           ID = "move-down"
	NextDisplay        ID = "next-display"
	PreviousDisplay    ID = "previous-display"
	ToggleFullscreen   ID = "toggle-fullscreen"
	PreviousSpace      ID = "previous-space"
	NextSpace          ID = "next-space"
)

type Kind uint8

const (
	KindGeometry Kind = iota
	KindRestore
	KindFullscreen
	KindSpace
)

type Group uint8

const (
	GroupHalves Group = iota
	GroupQuarters
	GroupFourths
	GroupThirds
	GroupSizing
	GroupMoving
	GroupFullscreen
	GroupSpaces
)

var All = []ID{
	LeftHalf, RightHalf, TopHalf, BottomHalf,
	TopLeftQuarter, TopRightQuarter, BottomLeftQuarter, BottomRightQuarter,
	FirstThreeFourths, LastThreeFourths,
	FirstThird, CenterThird, LastThird, FirstTwoThirds, LastTwoThirds,
	Maximize, AlmostMaximize, ReasonableSize, MaximizeHeight, MaximizeWidth,
	Center, CenterHalf, MakeLarger, MakeSmaller, Restore,
	MoveLeft, MoveRight, MoveUp, MoveDown, NextDisplay, PreviousDisplay,
	ToggleFullscreen, PreviousSpace, NextSpace,
}

func (id ID) EntryID() string { return "window-command:" + string(id) }

func FromEntryID(entryID string) (ID, bool) {
	const prefix = "window-command:"
	if len(entryID) <= len(prefix) || entryID[:len(prefix)] != prefix {
		return "", false
	}
	value := ID(entryID[len(prefix):])
	for _, candidate := range All {
		if candidate == value {
			return value, true
		}
	}
	return "", false
}

func (id ID) Kind() Kind {
	switch id {
	case Restore:
		return KindRestore
	case ToggleFullscreen:
		return KindFullscreen
	case PreviousSpace, NextSpace:
		return KindSpace
	}
	return KindGeometry
}

func (id ID) Group() Group {
	switch id {
	case LeftHalf, RightHalf, TopHalf, BottomHalf:
		return GroupHalves
	case TopLeftQuarter, TopRightQuarter, BottomLeftQuarter, BottomRightQuarter:
		return GroupQuarters
	case FirstThreeFourths, LastThreeFourths:
		return GroupFourths
	case FirstThird, CenterThird, LastThird, FirstTwoThirds, LastTwoThirds:
		return GroupThirds
	case Maximize, AlmostMaximize, ReasonableSize, MaximizeHeight, MaximizeWidth,
		Center, CenterHalf, MakeLarger, MakeSmaller, Restore:
		return GroupSizing
	case MoveLeft, MoveRight, MoveUp, MoveDown, NextDisplay, PreviousDisplay:
		return GroupMoving
	case ToggleFullscreen:
		return GroupFullscreen
	case PreviousSpace, NextSpace:
		return GroupSpaces
	}
	return GroupHalves
}

func (id ID) CyclesOnRepeat() bool {
	switch id {
	case LeftHalf, RightHalf, TopHalf, BottomHalf:
		return true
	}
	return false
}

func (id ID) Resizes() bool {
	switch id {
	case MoveLeft, MoveRight, MoveUp, MoveDown:
		return false
	}
	return true
}

func GroupTitle(group Group, chinese bool) string {
	if chinese {
		switch group {
		case GroupHalves:
			return "半屏"
		case GroupQuarters:
			return "四分之一"
		case GroupFourths:
			return "四分之三"
		case GroupThirds:
			return "三等分"
		case GroupSizing:
			return "尺寸"
		case GroupMoving:
			return "移动"
		case GroupFullscreen:
			return "全屏"
		case GroupSpaces:
			return "虚拟桌面"
		}
		return ""
	}
	switch group {
	case GroupHalves:
		return "Halves"
	case GroupQuarters:
		return "Quarters"
	case GroupFourths:
		return "Fourths"
	case GroupThirds:
		return "Thirds"
	case GroupSizing:
		return "Sizing"
	case GroupMoving:
		return "Moving"
	case GroupFullscreen:
		return "Fullscreen"
	case GroupSpaces:
		return "Spaces"
	}
	return ""
}

func (id ID) Name(chinese bool) string {
	if chinese {
		switch id {
		case LeftHalf:
			return "左半屏"
		case RightHalf:
			return "右半屏"
		case TopHalf:
			return "上半屏"
		case BottomHalf:
			return "下半屏"
		case TopLeftQuarter:
			return "左上四分之一"
		case TopRightQuarter:
			return "右上四分之一"
		case BottomLeftQuarter:
			return "左下四分之一"
		case BottomRightQuarter:
			return "右下四分之一"
		case FirstThreeFourths:
			return "左侧四分之三"
		case LastThreeFourths:
			return "右侧四分之三"
		case FirstThird:
			return "左侧三分之一"
		case CenterThird:
			return "中间三分之一"
		case LastThird:
			return "右侧三分之一"
		case FirstTwoThirds:
			return "左侧三分之二"
		case LastTwoThirds:
			return "右侧三分之二"
		case Maximize:
			return "最大化"
		case AlmostMaximize:
			return "接近最大化"
		case ReasonableSize:
			return "合适尺寸"
		case MaximizeHeight:
			return "最大化高度"
		case MaximizeWidth:
			return "最大化宽度"
		case Center:
			return "居中"
		case CenterHalf:
			return "中间半屏"
		case MakeLarger:
			return "放大"
		case MakeSmaller:
			return "缩小"
		case Restore:
			return "还原窗口"
		case MoveLeft:
			return "向左移动"
		case MoveRight:
			return "向右移动"
		case MoveUp:
			return "向上移动"
		case MoveDown:
			return "向下移动"
		case NextDisplay:
			return "移到下一个显示器"
		case PreviousDisplay:
			return "移到上一个显示器"
		case ToggleFullscreen:
			return "切换全屏"
		case PreviousSpace:
			return "切换到上一个虚拟桌面"
		case NextSpace:
			return "切换到下一个虚拟桌面"
		}
		return string(id)
	}
	switch id {
	case LeftHalf:
		return "Left Half"
	case RightHalf:
		return "Right Half"
	case TopHalf:
		return "Top Half"
	case BottomHalf:
		return "Bottom Half"
	case TopLeftQuarter:
		return "Top Left Quarter"
	case TopRightQuarter:
		return "Top Right Quarter"
	case BottomLeftQuarter:
		return "Bottom Left Quarter"
	case BottomRightQuarter:
		return "Bottom Right Quarter"
	case FirstThreeFourths:
		return "First Three Fourths"
	case LastThreeFourths:
		return "Last Three Fourths"
	case FirstThird:
		return "First Third"
	case CenterThird:
		return "Center Third"
	case LastThird:
		return "Last Third"
	case FirstTwoThirds:
		return "First Two Thirds"
	case LastTwoThirds:
		return "Last Two Thirds"
	case Maximize:
		return "Maximize"
	case AlmostMaximize:
		return "Almost Maximize"
	case ReasonableSize:
		return "Reasonable Size"
	case MaximizeHeight:
		return "Maximize Height"
	case MaximizeWidth:
		return "Maximize Width"
	case Center:
		return "Center"
	case CenterHalf:
		return "Center Half"
	case MakeLarger:
		return "Make Larger"
	case MakeSmaller:
		return "Make Smaller"
	case Restore:
		return "Restore Window"
	case MoveLeft:
		return "Move Left"
	case MoveRight:
		return "Move Right"
	case MoveUp:
		return "Move Up"
	case MoveDown:
		return "Move Down"
	case NextDisplay:
		return "Move to Next Display"
	case PreviousDisplay:
		return "Move to Previous Display"
	case ToggleFullscreen:
		return "Toggle Fullscreen"
	case PreviousSpace:
		return "Switch to Previous Space"
	case NextSpace:
		return "Switch to Next Space"
	}
	return string(id)
}
