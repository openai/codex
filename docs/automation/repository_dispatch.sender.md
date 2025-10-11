# Repository Dispatch Sender Guide

To trigger the Agent Bus via `repository_dispatch`, send a POST to:

  POST https://api.github.com/repos/{owner}/{repo}/dispatches

Headers:
- Authorization: token <PAT with repo:write>
- Accept: application/vnd.github+json

Body:
```
{
  "event_type": "agent-event",
  "client_payload": {
    "command": "/notify",
    "payload": { "source": "external", "msg": "hello" },
    "idempotency_key": "sha256:...",  // optional
    "signature": "<hex sha256>"       // HMAC over compacted client_payload without the signature field
  }
}
```

Signature computation (bash):
```
PAYLOAD_COMPACT=$(jq -c '.' client_payload.json)
printf "%s" "$PAYLOAD_COMPACT" | openssl dgst -sha256 -hmac "$REPO_DISPATCH_SECRET" -binary | xxd -p -c 256
```

The workflow `.github/workflows/agent_bus_dispatch.yml` verifies the signature using `REPO_DISPATCH_SECRET` and only accepts `event_type = agent-event`.

