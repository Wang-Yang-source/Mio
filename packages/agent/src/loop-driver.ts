/** Experimental native scheduling protocol. No messages or JS objects cross this boundary. */
export type LoopAction =
	| "steering"
	| "prepare"
	| "request"
	| "tools"
	| "truncated_tools"
	| "finish"
	| "follow_up"
	| "end";

export type LoopFeedback =
	| "empty"
	| "messages"
	| "prepared"
	| "response"
	| "response_error"
	| "tool_calls"
	| "truncated_tool_calls"
	| "tools_done"
	| "tools_terminate"
	| "auto"
	| "continue"
	| "end";

export interface LoopDriver {
	action(): LoopAction;
	advance(feedback: LoopFeedback): LoopAction;
	close(): void;
}
