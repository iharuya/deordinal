export type ViolationType = "doc" | "code_comment";

export type Violation = {
	line: number;
	text: string;
	type: ViolationType;
	reason: string;
};
