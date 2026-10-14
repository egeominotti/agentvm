"""Translate real CLI events into the dashboard's conversation wire format."""
import json


def message(blocks, role="assistant"):
    return {"type": role, "message": {"role": role, "content": blocks}}


def text(value):
    return message([{"type": "text", "text": str(value)}])


def tool(part):
    state = part.get("state") or {}
    name = part.get("tool") or part.get("name") or part.get("tool_name") or "command"
    identifier = part.get("callID") or part.get("id") or part.get("tool_id") or name
    inputs = state.get("input") or part.get("input") or part.get("parameters") or {}
    result = [message([{"type": "tool_use", "id": identifier, "name": name, "input": inputs}])]
    output = state.get("output", state.get("error", part.get("output")))
    if output is not None:
        result.append(message([{"type": "tool_result", "tool_use_id": identifier,
                                "is_error": state.get("status") == "error", "content": output}], "user"))
    return result


def normalize(kind, event):
    if kind == "claude" and isinstance(event, dict):
        return [event]
    if not isinstance(event, dict):
        return [text(event)]
    category = event.get("type", "")
    if kind == "copilot":
        data = event.get("data") or {}
        if category == "assistant.message":
            return [text(data.get("content", ""))]
        if category in ("assistant.message_delta", "assistant.usage"):
            # Completed messages retain the same content once, without duplicate deltas.
            return []
        if category == "tool.execution_start":
            return tool({"id": data.get("toolCallId"), "name": data.get("toolName"),
                         "input": data.get("arguments")})
        if category == "tool.execution_complete":
            output = (data.get("result") or {}).get("content") or (data.get("error") or {}).get("message", "")
            return [message([{"type": "tool_result", "tool_use_id": data.get("toolCallId", "tool"),
                              "is_error": not data.get("success", False), "content": output}], "user")]
    if kind == "goose" and category == "message":
        nested = event.get("message") or {}
        blocks = nested.get("content") or []
        readable = [block for block in blocks if block.get("type") == "text"]
        unknown = [block for block in blocks if block.get("type") != "text"]
        result = [message(readable, nested.get("role", "assistant"))] if readable else []
        result.extend(text(json.dumps(block, ensure_ascii=False)) for block in unknown)
        return result
    if kind == "codex":
        if category == "item.completed":
            item = event.get("item") or {}
            if item.get("type") in ("agent_message", "reasoning"):
                return [text(item.get("text", ""))]
            if item.get("type") == "command_execution":
                return tool({"id": item.get("id"), "name": "command",
                             "input": {"command": item.get("command", "")},
                             "output": item.get("aggregated_output", "")})
        if category in ("thread.started", "turn.started", "turn.completed", "item.started", "item.updated"):
            return []
    if category in ("assistant", "user") and isinstance(event.get("message"), dict):
        # Qwen's stream also uses Claude-shaped messages, without Claude billing.
        return [{"type": category, "message": event["message"]}]
    if category == "message" and event.get("role") == "assistant":
        content = event.get("content", "")
        return [message(content)] if isinstance(content, list) else [text(content)]
    if category in ("text", "reasoning") and isinstance(event.get("part"), dict):
        return [text(event["part"].get("text", ""))]
    if category == "tool_use":
        return tool(event.get("part") or event)
    if category == "tool_result":
        return [message([{"type": "tool_result", "tool_use_id": event.get("tool_id", "tool"),
                          "is_error": event.get("status") == "error",
                          "content": event.get("output", event.get("error", ""))}], "user")]
    if category == "result":
        result = event.get("result") or event.get("response") or event.get("error")
        return [text(result)] if result else []
    if category in ("init", "step_start", "step_finish", "complete", "usage"):
        return []
    # Preserve unfamiliar events so new CLI versions do not silently lose output.
    return [text(json.dumps(event, ensure_ascii=False))]
