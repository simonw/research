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

/** Action types the explorer agent can take. */
export type ActionType = 'click' | 'type' | 'scroll' | 'navigate' | 'wait' | 'select' | 'press';

/** An action with intent metadata (Skyvern-style). */
export interface ActionWithIntent {
  step: number;
  action: ActionType;
  /** Element ref from snapshot (e.g. "e3"). */
  ref?: string;
  /** Text to type (for 'type' actions). */
  text?: string;
  /** URL to navigate to (for 'navigate' actions). */
  url?: string;
  /** Key to press (for 'press' actions). */
  key?: string;
  /** LLM's reasoning for this action. */
  reasoning: string;
  /** What the LLM intends to achieve. */
  intention: string;
  /** LLM's confidence in this action (0-1). */
  confidence?: number;
  /** Timestamp when action was executed. */
  timestamp: number;
}

/** An API call observed during exploration. */
export interface ObservedApiCall {
  url: string;
  method: string;
  status: number;
  contentType?: string;
  bodySize?: number;
  timestamp: number;
}

/** User action from recording (for correlation). */
export interface UserAction {
  type: ActionType;
  timestamp: string;
  url: string;
  selector?: string;
  value?: string;
  tagName?: string;
  text?: string;
  key?: string;
  description?: string;
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
  triggerAction?: string;
}

/** Structured execution plan from the planning phase. */
export interface ExecutionPlan {
  taskSummary: string;
  steps: PlanStep[];
}

/** Parsed intent from natural language. */
export interface ParsedIntent {
  domain: string;
  url: string;
  task: string;
  requiresAuth: boolean;
}

/** LLM explore step decision. */
export interface ExploreDecision {
  done: boolean;
  action?: ActionType;
  ref?: string;
  text?: string;
  url?: string;
  key?: string;
  reasoning: string;
  intention: string;
  confidence?: number;
}

/** Explore session result. */
export interface ExploreResult {
  harPath: string;
  actions: ActionWithIntent[];
  apisSeen: ObservedApiCall[];
  sessionDir: string;
}

/** Analysis result from the analysis pipeline. */
export interface AnalysisResult {
  ir: RunIr;
  harSummary: string;
  timeline: ActionApiTimeline;
  workflow: WorkflowAnalysis;
  requestTemplates: RequestTemplate[];
}

// --- IR Types ---

export const IR_VERSION = 1 as const;

export type HttpMethod = 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE' | 'HEAD' | 'OPTIONS';

export interface EndpointVariant {
  exampleUrl: string;
  status: number;
  contentType?: string;
  requestBodySample?: string;
  responseBodySample?: string;
  requestHeadersSample?: Record<string, string>;
}

export interface EndpointGroup {
  id: string;
  method: HttpMethod;
  domain: string;
  pathPattern: string;
  callCount: number;
  score: number;
  scoreReasons: string[];
  isHtmlLike: boolean;
  isJsonLike: boolean;
  isApiLike: boolean;
  variants: EndpointVariant[];
}

export interface AuthProfile {
  authMethod: string;
  cookieNames: string[];
  authHeaderNames: string[];
}

export interface RunIr {
  irVersion: typeof IR_VERSION;
  seedUrl: string;
  seedDomain: string;
  createdAt: string;
  auth: AuthProfile;
  endpoints: EndpointGroup[];
}

// --- Request Templates ---

export interface RequestTemplate {
  id: string;
  method: string;
  url: string;
  headers: Record<string, string>;
  body?: string;
  contentTypeHint?: string;
}

// --- Script Generation ---

export interface GenerationResult {
  script: string;
  explanation: string;
  apiEndpoints: string[];
  strategy: string;
  promptVersion?: string;
  irHash?: string;
  templatesHash?: string;
  executionPlan?: ExecutionPlan;
}

// --- Validation ---

export interface RunResult {
  success: boolean;
  exitCode: number;
  stdout: string;
  stderr: string;
  outputValid: boolean;
  outputItemCount: number | null;
  durationMs: number;
}

export interface IterationRecord {
  iteration: number;
  exitCode: number;
  errorSummary: string;
  outputValid: boolean;
  durationMs: number;
}

// --- Registry ---

export interface RegistryEntry {
  domain: string;
  task: string;
  scriptPath: string;
  createdAt: string;
  lastRunAt?: string;
  runCount: number;
  lastRunSuccess?: boolean;
  lastRunItemCount?: number;
}

export interface Registry {
  version: number;
  entries: RegistryEntry[];
}

// --- Auth ---

export interface AuthProfileMeta {
  profileName: string;
  domain: string;
  createdAt: string;
  updatedAt?: string;
}

// --- LLM Provider ---

export interface LlmProvider {
  name: string;
  generateText(systemPrompt: string, userPrompt: string): Promise<string>;
  generateJson<T>(systemPrompt: string, userPrompt: string): Promise<T>;
}
