## Overview
`chatwidget::session_header` stores and updates the model name displayed at the top of the transcript. It provides a simple container for `ChatWidget` to reference when rendering session metadata.

## Notes
- `SessionHeader::set_model` updates the stored string when the session-configured event reports a new model.
