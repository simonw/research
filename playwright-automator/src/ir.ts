/**
 * Intermediate Representation (IR) for a recording run.
 *
 * Goal: deterministic, versioned summary of network + auth signals used for codegen.
 */

export const IR_VERSION = 1 as const;

export type HttpMethod = 'GET'|'POST'|'PUT'|'PATCH'|'DELETE'|'HEAD'|'OPTIONS';

export interface EndpointVariant {
  /** Full example URL (including querystring if present). */
  exampleUrl: string;
  /** Example status code. */
  status: number;
  /** Example content-type. */
  contentType?: string;
  /** Example request body (truncated). */
  requestBodySample?: string;
  /** Example response body (truncated). */
  responseBodySample?: string;
  /** Example request headers (pseudo-headers removed). */
  requestHeadersSample?: Record<string,string>;
}

export interface EndpointGroup {
  id: string;
  method: HttpMethod;
  domain: string;
  /** Normalized path (ids replaced). */
  pathPattern: string;
  /** Number of calls captured. */
  callCount: number;
  /** Heuristic score: higher = more likely useful for extraction. */
  score: number;
  /** Why the score is what it is (for debugging + LLM guidance). */
  scoreReasons: string[];
  /** Whether response looked like HTML (usually noise). */
  isHtmlLike: boolean;
  /** Whether response looked like JSON. */
  isJsonLike: boolean;
  /** Whether URL looks like API-ish (api/graphql/v1 etc). */
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
