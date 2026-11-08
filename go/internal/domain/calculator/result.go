package calculator

// Result is one calculator answer card. An empty badge pair marks an error.
type Result struct {
	Expression  string
	Display     string
	CopyText    string
	SourceBadge string
	TargetBadge string
}

func (r Result) IsError() bool { return r.SourceBadge == "" && r.TargetBadge == "" }

func valueResult(expression, display, copyText, sourceBadge, targetBadge string) *Result {
	return &Result{Expression: expression, Display: display, CopyText: copyText,
		SourceBadge: sourceBadge, TargetBadge: targetBadge}
}

func errorResult(expression, message string) *Result {
	return &Result{Expression: expression, Display: message, CopyText: message}
}

func (r *Result) withExpression(expression string) *Result {
	r.Expression = expression
	return r
}
