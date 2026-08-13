import json
import unittest

from benchmark_luna_websocket import (
    ENCODING,
    Sample,
    build_request,
    constrained_score,
    make_prompt,
    percentile,
    retry_delay_seconds,
    synthetic_prompt_tokens,
)


class LunaWebsocketBenchmarkTest(unittest.TestCase):
    def test_synthetic_prompt_hits_exact_token_target(self) -> None:
        corpus = synthetic_prompt_tokens(512)
        prompt = make_prompt(256, 7, corpus)
        self.assertEqual(len(ENCODING.encode(prompt)), 256)
        self.assertTrue(prompt.startswith("Unique benchmark sample 7;"))

    def test_request_requires_exactly_one_grammar_constrained_digit(self) -> None:
        sample = Sample(3, 256, "xhigh", "synthetic agent activity")
        request = json.loads(build_request(sample, "test-model", 1024))
        self.assertEqual(request["reasoning"], {"effort": "xhigh"})
        self.assertEqual(request["tool_choice"], "required")
        self.assertEqual(request["tools"][0]["format"]["definition"], "[1-9]")

    def test_constrained_score_rejects_unexpected_outputs(self) -> None:
        response = {
            "output": [{"type": "custom_tool_call", "name": "risk_score", "input": "7"}]
        }
        self.assertEqual(constrained_score(response), 7)
        response["output"][0]["input"] = "10"
        with self.assertRaises(ValueError):
            constrained_score(response)

    def test_percentiles_interpolate_small_samples(self) -> None:
        self.assertEqual(percentile([1, 2, 3, 4], 0.50), 2.5)
        self.assertEqual(percentile([1, 2, 3, 4], 0.95), 3.85)

    def test_retry_backoff_honors_server_rate_limit_delay(self) -> None:
        self.assertEqual(retry_delay_seconds("Please try again in 750ms", 2), 1.75)
        self.assertEqual(retry_delay_seconds("Please try again in 1.25s", 1), 2.0)


if __name__ == "__main__":
    unittest.main()
