"""DeepSeek chat client, via the OpenAI SDK pointed at DeepSeek's OpenAI-compatible API.

Models (September 2026): `deepseek-flash` (DeepSeek-V4.1-Flash) and `deepseek-v4-pro`.
Thinking mode is on by default; `reasoning_effort` is low | high | max.

One rule matters for agents: when `tools` are sent in thinking mode, every earlier
assistant message must carry its `reasoning_content` back to the API, or it returns 400.
`Reply.as_message()` keeps it.
"""

import os
from dataclasses import dataclass, field

DEEPSEEK_BASE_URL = "https://api.deepseek.com"
CLERK_MODEL = "deepseek-flash"
REVIEWER_MODEL = "deepseek-v4-pro"


class LLMError(Exception):
    def __init__(self, message, status=None, retryable=False):
        super().__init__(message)
        self.status = status
        self.retryable = retryable


@dataclass
class ToolCall:
    id: str
    name: str
    arguments: str  # JSON text, as written by the model


@dataclass
class Reply:
    content: str
    reasoning_content: str
    tool_calls: list
    finish_reason: str
    usage: dict = field(default_factory=dict)

    def as_message(self):
        """The assistant message to append to the conversation, reasoning included."""
        message = {"role": "assistant", "content": self.content or "", "reasoning_content": self.reasoning_content or ""}
        if self.tool_calls:
            message["tool_calls"] = [
                {"id": c.id, "type": "function", "function": {"name": c.name, "arguments": c.arguments}}
                for c in self.tool_calls
            ]
        return message


class DeepSeek:
    def __init__(self, api_key=None, base_url=None, timeout=300.0, max_retries=3):
        import openai

        self._openai = openai
        key = api_key or os.environ.get("DEEPSEEK_API_KEY")
        if not key:
            raise LLMError("DEEPSEEK_API_KEY is not set")
        self.client = openai.OpenAI(api_key=key, base_url=base_url or os.environ.get("DEEPSEEK_BASE_URL", DEEPSEEK_BASE_URL),
                                    timeout=timeout, max_retries=max_retries)

    def chat(self, *, model, messages, tools=None, thinking=True, effort="high", json_output=False):
        kwargs = {
            "model": model,
            "messages": messages,
            "extra_body": {"thinking": {"type": "enabled" if thinking else "disabled"}},
        }
        if thinking:
            kwargs["reasoning_effort"] = effort
        if tools:
            kwargs["tools"] = tools
            kwargs["tool_choice"] = "auto"
        if json_output:
            kwargs["response_format"] = {"type": "json_object"}

        openai = self._openai
        try:
            response = self.client.chat.completions.create(**kwargs)
        except openai.AuthenticationError as e:
            raise LLMError(f"DeepSeek rejected the API key: {e.message}", status=401) from None
        except openai.BadRequestError as e:
            raise LLMError(f"DeepSeek rejected the request: {e.message}", status=400) from None
        except openai.RateLimitError as e:
            raise LLMError(f"rate limited or out of balance: {e.message}", status=429, retryable=True) from None
        except openai.APIStatusError as e:
            raise LLMError(f"DeepSeek API error {e.status_code}: {e.message}", status=e.status_code,
                           retryable=e.status_code >= 500) from None
        except openai.APIConnectionError as e:
            raise LLMError(f"could not reach DeepSeek: {e}", retryable=True) from None

        choice = response.choices[0]
        message = choice.message
        usage = response.usage.model_dump() if response.usage else {}
        return Reply(
            content=message.content or "",
            reasoning_content=getattr(message, "reasoning_content", None) or "",
            tool_calls=[ToolCall(c.id, c.function.name, c.function.arguments) for c in (message.tool_calls or [])],
            finish_reason=choice.finish_reason,
            usage=usage,
        )
