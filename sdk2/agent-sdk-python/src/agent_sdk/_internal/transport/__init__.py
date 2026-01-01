"""Transport implementations for Codex SDK."""

from abc import ABC, abstractmethod
from collections.abc import AsyncIterator
from typing import Any


class Transport(ABC):
    """Abstract transport for Codex communication.

    WARNING: This internal API is exposed for custom transport implementations
    (e.g., remote Codex connections). The Codex team may change or
    or remove this abstract class in any future release. Custom implementations
    must be updated to match interface changes.

    This is a low-level transport interface that handles raw I/O with the Codex
    process or service. The Query class builds on top of this to implement the
    control protocol and message routing.
    """

    @abstractmethod
    async def connect(self) -> None:
        """Connect the transport and prepare for communication.

        For subprocess transports, this starts the process.
        For network transports, this establishes the connection.
        """
        pass

    @abstractmethod
    async def write(self, data: str) -> None:
        """Write raw data to the transport.

        Args:
            data: Raw string data to write (typically JSON + newline)
        """
        pass

    @abstractmethod
    def read_messages(self) -> AsyncIterator[dict[str, Any]]:
        """Read and parse messages from the transport.

        Yields:
            Parsed JSON messages from the transport
        """
        pass

    @abstractmethod
    async def close(self) -> None:
        """Close the transport connection and clean up resources."""
        pass

    @abstractmethod
    def is_ready(self) -> bool:
        """Check if transport is ready for communication.

        Returns:
            True if transport is ready to send/receive messages
        """
        pass

    @abstractmethod
    async def end_input(self) -> None:
        """End the input stream (close stdin for process transports)."""
        pass


__all__ = ["Transport"]
