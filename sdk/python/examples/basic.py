from codex_app_server import AppServerClient


with AppServerClient() as client:
    client.initialize()

    thread = client.thread_start(model="gpt-5")
    thread_id = thread["thread"]["id"]

    turn = client.turn_start(
        thread_id,
        input_items=[{"type": "text", "text": "Explain gradient descent briefly."}],
    )

    done = client.wait_for_turn_completed(turn["turn"]["id"])
    print("Turn status:", done.params["turn"]["status"])
