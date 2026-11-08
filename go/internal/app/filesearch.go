package app

import (
	"log"

	domainfiles "github.com/YYHCOPPOLO/wincast/go/internal/domain/filesearch"
	files "github.com/YYHCOPPOLO/wincast/go/internal/features/filesearch"
	"github.com/YYHCOPPOLO/wincast/go/internal/platform/windows"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/components"
	"github.com/egoist/mygo"
)

func (c *controller) openFileSearch() {
	if c.closing || !c.featureFileSearch {
		return
	}
	if c.fileSearch != nil {
		c.fileSearch.Apply(c.fileSearchScopes, c.fileSearchIgnores)
	}
	c.view.SetModeFileSearch()
	if !c.panel.IsVisible() {
		c.target = windows.ForegroundTarget()
	}
	c.showPanel(false)
}

// startFileSearch is called by the view for every query change. The session
// debounces, cancels the previous request and drops superseded revisions.
func (c *controller) startFileSearch(query string) {
	if c.fileSearch == nil {
		return
	}
	revision := c.fileSearch.Search(c.ctx, query, func(rev uint64, state files.State, hits []domainfiles.Hit, err error) {
		if c.closing || c.panel == nil {
			return
		}
		message := ""
		if err != nil {
			message = c.text("搜索文件失败", "File search failed")
			log.Printf("file search revision=%d error=%v", rev, err)
		}
		c.panel.Update(func() {
			if c.closing || rev != c.fileSearchRevision {
				return
			}
			if state == files.Searching {
				c.view.FileSearchState = files.Searching
				return
			}
			c.view.SetFileSearchResults(state, hits, message)
		})
	})
	c.fileSearchRevision = revision
}

func (c *controller) selectedFile() (domainfiles.Hit, bool) {
	if c.view == nil || c.view.Chosen < 0 || c.view.Chosen >= len(c.view.FileSearchItems) {
		return domainfiles.Hit{}, false
	}
	return c.view.FileSearchItems[c.view.Chosen], true
}

func (c *controller) openFileHit(hit domainfiles.Hit) {
	if c.closing || hit.Path == "" {
		return
	}
	c.hide()
	if err := mygo.Shell.OpenPath(hit.Path); err != nil {
		log.Printf("open file failed: %v", err)
		c.showFailure(c.text("无法打开文件", "Couldn’t open file"), hit.Name, err.Error())
	}
}

func (c *controller) revealFileHit(hit domainfiles.Hit) {
	if c.closing || hit.Path == "" {
		return
	}
	c.hide()
	mygo.Shell.ShowItemInFolder(hit.Path)
}

func (c *controller) copyFileHitPath(hit domainfiles.Hit) {
	if hit.Path == "" {
		return
	}
	c.copyText(hit.Path, false)
}

func (c *controller) fileSearchMenu() {
	selected, ok := c.selectedFile()
	menu := components.Menu([]components.MenuAction{
		{Label: c.text("打开", "Open"), Accelerator: "↵", Disabled: !ok, Click: func() {
			if ok {
				c.openFileHit(selected)
			}
		}},
		{Label: c.text("在文件夹中显示", "Show in folder"), Disabled: !ok, Click: func() {
			if ok {
				c.revealFileHit(selected)
			}
		}},
		{Label: c.text("复制路径", "Copy path"), Accelerator: "Ctrl+Shift+C", Disabled: !ok, Click: func() {
			if ok {
				c.copyFileHitPath(selected)
			}
		}},
		{Separator: true},
		{Label: c.text("返回启动器", "Back to launcher"), Accelerator: "Esc", Click: c.view.SetModeLauncher},
		{Label: c.text("设置…", "Settings…"), Click: c.openSettings},
		{Label: c.text("退出预览", "Quit Preview"), Click: mygo.App.Quit},
	})
	menu.Popup(c.panel)
}
