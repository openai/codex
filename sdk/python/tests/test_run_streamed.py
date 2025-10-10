from __future__ import annotations

from codex import TurnOptions
from codex.events import (
    ItemCompletedEvent,
    ThreadStartedEvent,
    TurnCompletedEvent,
    TurnStartedEvent,
)

from .helpers import (
    assistant_message,
    response_completed,
    response_started,
    sse,
    start_responses_proxy,
)


def collect_events(stream) -> list:
    events = list(stream)
    return events


def test_returns_thread_events(codex_client) -> None:
    proxy = start_responses_proxy([sse(response_started(), assistant_message("Hi!"), response_completed())])
    try:
        client = codex_client(proxy.url)
        thread = client.start_thread()
        stream = thread.run_streamed("Hello, world!")
        events = collect_events(stream)

        assert len(events) == 4
        assert isinstance(events[0], ThreadStartedEvent)
        assert thread.id is not None
        assert isinstance(events[1], TurnStartedEvent)
        item_event = events[2]
        assert isinstance(item_event, ItemCompletedEvent)
        assert item_event.item.type == "agent_message"
        assert item_event.item.text == "Hi!"
        completed = events[3]
        assert isinstance(completed, TurnCompletedEvent)
        assert completed.usage.cached_input_tokens == 12
    finally:
        proxy.close()


def test_sends_previous_items_on_streamed_run(codex_client) -> None:
    proxy = start_responses_proxy(
        [
            sse(response_started("response_1"), assistant_message("First response", "item_1"), response_completed("response_1")),
            sse(response_started("response_2"), assistant_message("Second response", "item_2"), response_completed("response_2")),
        ]
    )
    try:
        client = codex_client(proxy.url)
        thread = client.start_thread()
        first = thread.run_streamed("first input")
        collect_events(first)

        second = thread.run_streamed("second input")
        collect_events(second)

        second_request = proxy.requests[1]
        assistant_entry = next(entry for entry in second_request["json"]["input"] if entry.get("role") == "assistant")
        assistant_text = next(content for content in assistant_entry["content"] if content["type"] == "output_text")
        assert assistant_text["text"] == "First response"
    finally:
        proxy.close()


def test_resumes_thread_by_id_when_streaming(codex_client) -> None:
    proxy = start_responses_proxy(
        [
            sse(response_started("response_1"), assistant_message("First response", "item_1"), response_completed("response_1")),
            sse(response_started("response_2"), assistant_message("Second response", "item_2"), response_completed("response_2")),
        ]
    )
    try:
        client = codex_client(proxy.url)
        original_thread = client.start_thread()
        collect_events(original_thread.run_streamed("first input"))

        assert original_thread.id is not None
        resumed_thread = client.resume_thread(original_thread.id)
        collect_events(resumed_thread.run_streamed("second input"))

        second_request = proxy.requests[1]
        assistant_entry = next(entry for entry in second_request["json"]["input"] if entry.get("role") == "assistant")
        assistant_text = next(content for content in assistant_entry["content"] if content["type"] == "output_text")
        assert assistant_text["text"] == "First response"
    finally:
        proxy.close()


def test_applies_output_schema_when_streaming(codex_client) -> None:
    proxy = start_responses_proxy([sse(response_started(), assistant_message("Structured"), response_completed())])
    schema = {
        "type": "object",
        "properties": {"answer": {"type": "string"}},
        "required": ["answer"],
        "additionalProperties": False,
    }
    try:
        client = codex_client(proxy.url)
        thread = client.start_thread()
        collect_events(thread.run_streamed("structured", TurnOptions(output_schema=schema)))

        payload = proxy.requests[0]["json"]
        assert payload["text"]["format"]["schema"] == schema
    finally:
        proxy.close()
