package codex

// ThreadStartOptions configures a thread/start request.
type ThreadStartOptions struct {
	Model                 string
	Cwd                   string
	ApprovalPolicy        any
	SandboxPolicy         any
	Config                map[string]any
	BaseInstructions      string
	DeveloperInstructions string
	ExperimentalRawEvents bool
}

func (o ThreadStartOptions) toParams() map[string]any {
	params := map[string]any{}
	if o.Model != "" {
		params["model"] = o.Model
	}
	if o.Cwd != "" {
		params["cwd"] = o.Cwd
	}
	if o.ApprovalPolicy != nil {
		params["approvalPolicy"] = o.ApprovalPolicy
	}
	if o.SandboxPolicy != nil {
		params["sandbox"] = o.SandboxPolicy
	}
	if o.Config != nil {
		params["config"] = o.Config
	}
	if o.BaseInstructions != "" {
		params["baseInstructions"] = o.BaseInstructions
	}
	if o.DeveloperInstructions != "" {
		params["developerInstructions"] = o.DeveloperInstructions
	}
	if o.ExperimentalRawEvents {
		params["experimentalRawEvents"] = true
	}
	return params
}

// ThreadResumeOptions configures a thread/resume request.
type ThreadResumeOptions struct {
	ThreadID              string
	History               []any
	Path                  string
	Model                 string
	ModelProvider         string
	Cwd                   string
	ApprovalPolicy        any
	Sandbox               any
	Config                map[string]any
	BaseInstructions      string
	DeveloperInstructions string
}

func (o ThreadResumeOptions) toParams() map[string]any {
	params := map[string]any{}
	if o.ThreadID != "" {
		params["threadId"] = o.ThreadID
	}
	if len(o.History) > 0 {
		params["history"] = o.History
	}
	if o.Path != "" {
		params["path"] = o.Path
	}
	if o.Model != "" {
		params["model"] = o.Model
	}
	if o.ModelProvider != "" {
		params["modelProvider"] = o.ModelProvider
	}
	if o.Cwd != "" {
		params["cwd"] = o.Cwd
	}
	if o.ApprovalPolicy != nil {
		params["approvalPolicy"] = o.ApprovalPolicy
	}
	if o.Sandbox != nil {
		params["sandbox"] = o.Sandbox
	}
	if o.Config != nil {
		params["config"] = o.Config
	}
	if o.BaseInstructions != "" {
		params["baseInstructions"] = o.BaseInstructions
	}
	if o.DeveloperInstructions != "" {
		params["developerInstructions"] = o.DeveloperInstructions
	}
	return params
}
