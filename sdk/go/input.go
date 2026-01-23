package codex

import "github.com/openai/codex/sdk/go/protocol"

// Input represents a structured user input message.
type Input struct {
	Type         string                 `json:"type"`
	Text         string                 `json:"text,omitempty"`
	TextElements []protocol.TextElement `json:"textElements,omitempty"`
	URL          string                 `json:"url,omitempty"`
	Path         string                 `json:"path,omitempty"`
	Name         string                 `json:"name,omitempty"`
}

// TextInput creates a text input entry.
func TextInput(text string) Input {
	return Input{Type: "text", Text: text}
}

// ImageInput creates a remote image input entry.
func ImageInput(url string) Input {
	return Input{Type: "image", URL: url}
}

// LocalImageInput creates a local image input entry.
func LocalImageInput(path string) Input {
	return Input{Type: "localImage", Path: path}
}

// SkillInput creates a skill input entry.
func SkillInput(name, path string) Input {
	return Input{Type: "skill", Name: name, Path: path}
}
