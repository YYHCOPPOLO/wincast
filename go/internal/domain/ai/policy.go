package ai

// ShouldResume reports whether summoning AI Chat resumes the last transcript.
// opensTo follows the saved number: nonzero means always start a new chat.
// afterMinutes < 0 means forever.
func ShouldResume(opensTo int64, afterMinutes int64, lastActivity, now int64) bool {
	if opensTo != 0 {
		return false
	}
	if lastActivity <= 0 || now < lastActivity {
		return false
	}
	if afterMinutes < 0 {
		return true
	}
	return now-lastActivity < afterMinutes*60
}
