class AppServerError(Exception):
    """Base exception for SDK errors."""


class JsonRpcError(AppServerError):
    def __init__(self, code: int, message: str, data=None):
        super().__init__(f"JSON-RPC error {code}: {message}")
        self.code = code
        self.message = message
        self.data = data


class TransportClosedError(AppServerError):
    """Raised when the app-server transport closes unexpectedly."""
