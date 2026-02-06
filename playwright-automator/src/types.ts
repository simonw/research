/** Raw HAR entry from a HAR file. */
export interface HarEntry {
  request: {
    method: string;
    url: string;
    headers: { name: string; value: string }[];
    cookies?: { name: string; value: string }[];
    queryString?: { name: string; value: string }[];
    postData?: { mimeType?: string; text?: string };
  };
  response: {
    status: number;
    headers: { name: string; value: string }[];
    content?: { size?: number; mimeType?: string; text?: string };
  };
  time?: number;
  startedDateTime?: string;
}

/** Parsed API request extracted from HAR. */
export interface ParsedApiRequest {
  method: string;
  url: string;
  path: string;
  domain: string;
  status: number;
  requestHeaders: Record<string, string>;
  requestBody?: string;
  responseContentType?: string;
  responseBody?: string;
  responseSize?: number;
  queryParams?: Record<string, string>;
  timestamp?: string;
}

/** Recorded user action in the browser. */
export interface UserAction {
  type: 'click' | 'type' | 'navigate' | 'scroll' | 'select' | 'press' | 'wait';
  timestamp: string;
  url: string;
  selector?: string;
  value?: string;
  tagName?: string;
  text?: string;
  key?: string;
  description?: string;
}

/** Full recording session data. */
export interface RecordingSession {
  id: string;
  url: string;
  description: string;
  startTime: string;
  endTime?: string;
  actions: UserAction[];
  apiRequests: ParsedApiRequest[];
  cookies: Record<string, string>;
  authHeaders: Record<string, string>;
  authMethod: string;
  targetDomain: string;
  harFilePath: string;
  screenshotsDir?: string;
  timeline?: ActionApiTimeline;
  workflowAnalysis?: WorkflowAnalysis;
  executionPlan?: ExecutionPlan;
}

/** An API call triggered by a user action. */
export interface TriggeredApi {
  method: string;
  url: string;
  path: string;
  status: number;
  delayMs: number;
  responseContentType?: string;
}

/** A user action correlated with the API calls it triggered. */
export interface CorrelatedAction {
  index: number;
  action: UserAction;
  triggeredApis: TriggeredApi[];
}

/** Timeline of correlated actions + uncorrelated background API calls. */
export interface ActionApiTimeline {
  correlatedActions: CorrelatedAction[];
  uncorrelatedApis: ParsedApiRequest[];
}

/** Detected workflow pattern. */
export interface WorkflowPattern {
  type: 'list-detail' | 'pagination' | 'variable-flow';
  description: string;
  confidence: 'high' | 'medium' | 'low';
  details: Record<string, unknown>;
}

/** Full workflow analysis result. */
export interface WorkflowAnalysis {
  patterns: WorkflowPattern[];
  summary: string;
}

/** A step in the execution plan. */
export interface PlanStep {
  step: number;
  description: string;
  endpoint: string;
  purpose: string;
  inputFrom?: number;
  loopOver?: string;
  /** UI action that triggers this API (e.g. "click conversation link in sidebar", "navigate to URL"). */
  triggerAction?: string;
}

/** Structured execution plan from the planning phase. */
export interface ExecutionPlan {
  taskSummary: string;
  steps: PlanStep[];
}

/** Configuration for the automator. */
export interface AutomatorConfig {
  geminiApiKey: string;
  outputDir: string;
  headless?: boolean;
  timeout?: number;
}

/** Result from Gemini script generation. */
export interface GenerationResult {
  script: string;
  explanation: string;
  apiEndpoints: string[];
  strategy: string;
  /** Prompt template version used for generation (for determinism/replay). */
  promptVersion?: string;
  /** Hash of the deterministic IR used as input. */
  irHash?: string;
  /** Hash of request templates used as input. */
  templatesHash?: string;
  /** Execution plan from the planning phase (if available). */
  executionPlan?: ExecutionPlan;
}
