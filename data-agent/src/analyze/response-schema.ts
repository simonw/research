/**
 * Response Schema Extractor — Replace blunt truncation with structural information.
 *
 * Instead of cutting off a 50KB JSON response at 5KB, extracts the type structure,
 * field names, sample values, and array lengths.
 */

interface SchemaNode {
  type: 'string' | 'number' | 'boolean' | 'null' | 'array' | 'object';
  sample?: string | number | boolean;
  arrayLength?: number;
  elementSchema?: SchemaNode;
  fields?: Record<string, SchemaNode>;
}

const MAX_DEPTH = 4;
const MAX_OBJECT_FIELDS = 30;

export function extractResponseSchema(body: string): SchemaNode | null {
  let parsed: unknown;
  try {
    const trimmed = body.trim();
    const cleaned = trimmed.startsWith('for (;;);') ? trimmed.slice(9) : trimmed;
    parsed = JSON.parse(cleaned);
  } catch {
    return null;
  }
  return extractNode(parsed, 0);
}

function extractNode(value: unknown, depth: number): SchemaNode {
  if (value === null || value === undefined) return { type: 'null' };

  if (depth >= MAX_DEPTH) {
    if (Array.isArray(value)) return { type: 'array', arrayLength: value.length };
    if (typeof value === 'object') return { type: 'object' };
    return { type: typeof value as SchemaNode['type'], sample: primitivePreview(value) };
  }

  if (Array.isArray(value)) {
    const node: SchemaNode = { type: 'array', arrayLength: value.length };
    if (value.length > 0) node.elementSchema = extractNode(value[0], depth + 1);
    return node;
  }

  if (typeof value === 'object') {
    const obj = value as Record<string, unknown>;
    const keys = Object.keys(obj);
    const fields: Record<string, SchemaNode> = {};
    for (const key of keys.slice(0, MAX_OBJECT_FIELDS)) {
      fields[key] = extractNode(obj[key], depth + 1);
    }
    if (keys.length > MAX_OBJECT_FIELDS) {
      fields[`... +${keys.length - MAX_OBJECT_FIELDS} more fields`] = { type: 'null' };
    }
    return { type: 'object', fields };
  }

  return { type: typeof value as SchemaNode['type'], sample: primitivePreview(value) };
}

function primitivePreview(value: unknown): string | number | boolean {
  if (typeof value === 'string') return value.length > 60 ? value.slice(0, 57) + '...' : value;
  if (typeof value === 'number' || typeof value === 'boolean') return value;
  return String(value);
}

export function renderSchemaForLLM(schema: SchemaNode, indent = 0): string {
  const pad = '  '.repeat(indent);

  if (schema.type === 'array') {
    const countStr = schema.arrayLength !== undefined ? `${schema.arrayLength} items` : '? items';
    if (!schema.elementSchema) return `${pad}Array[${countStr}] (empty)`;
    if (schema.elementSchema.type === 'object') {
      return `${pad}Array[${countStr}] of {\n${renderObjectFields(schema.elementSchema, indent + 1)}\n${pad}}`;
    }
    return `${pad}Array[${countStr}] of ${renderSchemaForLLM(schema.elementSchema, 0).trim()}`;
  }

  if (schema.type === 'object') {
    if (!schema.fields || Object.keys(schema.fields).length === 0) return `${pad}{}`;
    return `${pad}{\n${renderObjectFields(schema, indent + 1)}\n${pad}}`;
  }

  const sampleStr = schema.sample !== undefined ? ` (e.g. ${JSON.stringify(schema.sample)})` : '';
  return `${pad}${schema.type}${sampleStr}`;
}

function renderObjectFields(schema: SchemaNode, indent: number): string {
  if (!schema.fields) return '';
  const pad = '  '.repeat(indent);
  const lines: string[] = [];

  for (const [key, fieldSchema] of Object.entries(schema.fields)) {
    if (key.startsWith('... +')) { lines.push(`${pad}${key}`); continue; }

    if (fieldSchema.type === 'array') {
      const countStr = fieldSchema.arrayLength !== undefined ? `${fieldSchema.arrayLength} items` : '? items';
      if (fieldSchema.elementSchema?.type === 'object') {
        lines.push(`${pad}${key}: Array[${countStr}] of {`);
        lines.push(renderObjectFields(fieldSchema.elementSchema, indent + 1));
        lines.push(`${pad}}`);
      } else if (fieldSchema.elementSchema) {
        lines.push(`${pad}${key}: Array[${countStr}] of ${renderSchemaForLLM(fieldSchema.elementSchema, 0).trim()}`);
      } else {
        lines.push(`${pad}${key}: Array[${countStr}] (empty)`);
      }
    } else if (fieldSchema.type === 'object') {
      if (fieldSchema.fields && Object.keys(fieldSchema.fields).length > 0) {
        lines.push(`${pad}${key}: {`);
        lines.push(renderObjectFields(fieldSchema, indent + 1));
        lines.push(`${pad}}`);
      } else {
        lines.push(`${pad}${key}: {}`);
      }
    } else {
      const sampleStr = fieldSchema.sample !== undefined ? ` (e.g. ${JSON.stringify(fieldSchema.sample)})` : '';
      lines.push(`${pad}${key}: ${fieldSchema.type}${sampleStr}`);
    }
  }

  return lines.join('\n');
}
