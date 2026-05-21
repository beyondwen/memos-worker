type TokenType =
  | "IDENT" | "STRING" | "NUMBER" | "BOOL"
  | "EQ" | "NE" | "LT" | "LE" | "GT" | "GE"
  | "AND" | "OR" | "NOT"
  | "LPAREN" | "RPAREN" | "COMMA"
  | "DOT" | "IN" | "EOF";

interface Token {
  type: TokenType;
  value: string;
  pos: number;
}

type ASTNode =
  | { type: "binary"; op: string; left: ASTNode; right: ASTNode }
  | { type: "unary"; op: string; operand: ASTNode }
  | { type: "field_access"; field: string; sub?: string }
  | { type: "literal"; value: string | number | boolean }
  | { type: "in"; field: ASTNode; values: ASTNode[] }
  | { type: "contains"; field: ASTNode; value: ASTNode }
  | { type: "starts_with"; field: ASTNode; value: ASTNode }
  | { type: "ends_with"; field: ASTNode; value: ASTNode };

const FIELD_MAP: Record<string, { column: string; type: "string" | "number" | "boolean" | "json" | "json_tag" }> = {
  content: { column: "memo.content", type: "string" },
  creator: { column: '"user".username', type: "string" },
  creator_id: { column: "memo.creator_id", type: "number" },
  creatorId: { column: "memo.creator_id", type: "number" },
  created_ts: { column: "memo.created_ts", type: "number" },
  createdTs: { column: "memo.created_ts", type: "number" },
  updated_ts: { column: "memo.updated_ts", type: "number" },
  updatedTs: { column: "memo.updated_ts", type: "number" },
  pinned: { column: "memo.pinned", type: "boolean" },
  visibility: { column: "memo.visibility", type: "string" },
  tag: { column: "json_each.value", type: "json_tag" },
  tags: { column: "json_each.value", type: "json_tag" },
  has_task_list: { column: "json_extract(memo.payload, '$.property.hasTaskList')", type: "boolean" },
  hasTaskList: { column: "json_extract(memo.payload, '$.property.hasTaskList')", type: "boolean" },
  has_link: { column: "json_extract(memo.payload, '$.property.hasLink')", type: "boolean" },
  hasLink: { column: "json_extract(memo.payload, '$.property.hasLink')", type: "boolean" },
  has_code: { column: "json_extract(memo.payload, '$.property.hasCode')", type: "boolean" },
  hasCode: { column: "json_extract(memo.payload, '$.property.hasCode')", type: "boolean" },
  has_incomplete_tasks: { column: "json_extract(memo.payload, '$.property.hasIncompleteTasks')", type: "boolean" },
  hasIncompleteTasks: { column: "json_extract(memo.payload, '$.property.hasIncompleteTasks')", type: "boolean" },
  row_status: { column: "memo.row_status", type: "string" },
  rowStatus: { column: "memo.row_status", type: "string" },
};

class Lexer {
  private pos = 0;
  constructor(private input: string) {}

  tokenize(): Token[] {
    const tokens: Token[] = [];
    while (this.pos < this.input.length) {
      this.skipWhitespace();
      if (this.pos >= this.input.length) break;

      const ch = this.input[this.pos];
      const startPos = this.pos;

      if (ch === "(") { tokens.push({ type: "LPAREN", value: "(", pos: startPos }); this.pos++; continue; }
      if (ch === ")") { tokens.push({ type: "RPAREN", value: ")", pos: startPos }); this.pos++; continue; }
      if (ch === ",") { tokens.push({ type: "COMMA", value: ",", pos: startPos }); this.pos++; continue; }
      if (ch === ".") { tokens.push({ type: "DOT", value: ".", pos: startPos }); this.pos++; continue; }

      if (ch === '"' || ch === "'") { tokens.push(this.readString(ch)); continue; }

      if (ch === "=" && this.peek(1) === "=") {
        tokens.push({ type: "EQ", value: "==", pos: startPos }); this.pos += 2; continue;
      }
      if (ch === "!" && this.peek(1) === "=") {
        tokens.push({ type: "NE", value: "!=", pos: startPos }); this.pos += 2; continue;
      }
      if (ch === "<" && this.peek(1) === "=") {
        tokens.push({ type: "LE", value: "<=", pos: startPos }); this.pos += 2; continue;
      }
      if (ch === ">" && this.peek(1) === "=") {
        tokens.push({ type: "GE", value: ">=", pos: startPos }); this.pos += 2; continue;
      }
      if (ch === "<") { tokens.push({ type: "LT", value: "<", pos: startPos }); this.pos++; continue; }
      if (ch === ">") { tokens.push({ type: "GT", value: ">", pos: startPos }); this.pos++; continue; }
      if (ch === "&" && this.peek(1) === "&") {
        tokens.push({ type: "AND", value: "&&", pos: startPos }); this.pos += 2; continue;
      }
      if (ch === "|" && this.peek(1) === "|") {
        tokens.push({ type: "OR", value: "||", pos: startPos }); this.pos += 2; continue;
      }
      if (ch === "!") { tokens.push({ type: "NOT", value: "!", pos: startPos }); this.pos++; continue; }

      if (ch === "-" || (ch >= "0" && ch <= "9")) {
        tokens.push(this.readNumber());
        continue;
      }

      if (this.isIdentChar(ch)) {
        tokens.push(this.readIdentOrKeyword());
        continue;
      }

      this.pos++;
    }
    tokens.push({ type: "EOF", value: "", pos: this.pos });
    return tokens;
  }

  private skipWhitespace(): void {
    while (this.pos < this.input.length && /\s/.test(this.input[this.pos])) this.pos++;
  }

  private peek(offset: number): string {
    return this.input[this.pos + offset] ?? "";
  }

  private readString(quote: string): Token {
    const startPos = this.pos;
    this.pos++;
    let value = "";
    while (this.pos < this.input.length && this.input[this.pos] !== quote) {
      if (this.input[this.pos] === "\\") { this.pos++; }
      value += this.input[this.pos];
      this.pos++;
    }
    this.pos++;
    return { type: "STRING", value, pos: startPos };
  }

  private readIdentOrKeyword(): Token {
    const startPos = this.pos;
    let value = "";
    while (this.pos < this.input.length && this.isIdentChar(this.input[this.pos])) {
      value += this.input[this.pos];
      this.pos++;
    }
    if (value === "true" || value === "false") return { type: "BOOL", value, pos: startPos };
    if (value === "in") return { type: "IN", value, pos: startPos };
    return { type: "IDENT", value, pos: startPos };
  }

  private readNumber(): Token {
    const startPos = this.pos;
    let value = "";
    if (this.input[this.pos] === "-") { value += "-"; this.pos++; }
    while (this.pos < this.input.length && this.input[this.pos] >= "0" && this.input[this.pos] <= "9") {
      value += this.input[this.pos];
      this.pos++;
    }
    return { type: "NUMBER", value, pos: startPos };
  }

  private isIdentChar(ch: string): boolean {
    return /[a-zA-Z0-9_]/.test(ch);
  }
}

class Parser {
  private pos = 0;
  constructor(private tokens: Token[]) {}

  parse(): ASTNode {
    const node = this.parseOr();
    if (this.current().type !== "EOF") {
      throw new Error(`Unexpected token '${this.current().value}' at position ${this.current().pos}`);
    }
    return node;
  }

  private parseOr(): ASTNode {
    let left = this.parseAnd();
    while (this.current().type === "OR") {
      this.advance();
      const right = this.parseAnd();
      left = { type: "binary", op: "||", left, right };
    }
    return left;
  }

  private parseAnd(): ASTNode {
    let left = this.parseNot();
    while (this.current().type === "AND") {
      this.advance();
      const right = this.parseNot();
      left = { type: "binary", op: "&&", left, right };
    }
    return left;
  }

  private parseNot(): ASTNode {
    if (this.current().type === "NOT") {
      this.advance();
      const operand = this.parseNot();
      return { type: "unary", op: "!", operand };
    }
    return this.parseComparison();
  }

  private parseComparison(): ASTNode {
    let left = this.parsePrimary();

    if (this.current().type === "DOT" && this.peekIs("IDENT")) {
      this.advance();
      const method = this.current();
      this.advance();
      if (method.value === "contains" || method.value === "startsWith" || method.value === "endsWith") {
        this.expect("LPAREN");
        const arg = this.parsePrimary();
        this.expect("RPAREN");
        const methodType = method.value === "contains" ? "contains" : method.value === "startsWith" ? "starts_with" : "ends_with";
        return { type: methodType, field: left, value: arg };
      }
      left = { type: "field_access", field: (left as { type: "field_access"; field: string }).field, sub: method.value };
    }

    if (this.current().type === "IN") {
      this.advance();
      this.expect("LPAREN");
      const values: ASTNode[] = [];
      values.push(this.parsePrimary());
      while (this.current().type === "COMMA") {
        this.advance();
        values.push(this.parsePrimary());
      }
      this.expect("RPAREN");
      return { type: "in", field: left, values };
    }

    const op = this.current();
    if (op.type === "EQ" || op.type === "NE" || op.type === "LT" || op.type === "LE" || op.type === "GT" || op.type === "GE") {
      this.advance();
      const right = this.parsePrimary();
      return { type: "binary", op: op.value, left, right };
    }

    return left;
  }

  private parsePrimary(): ASTNode {
    const tok = this.current();

    if (tok.type === "LPAREN") {
      this.advance();
      const node = this.parseOr();
      this.expect("RPAREN");
      return node;
    }

    if (tok.type === "STRING") {
      this.advance();
      return { type: "literal", value: tok.value };
    }

    if (tok.type === "NUMBER") {
      this.advance();
      return { type: "literal", value: Number(tok.value) };
    }

    if (tok.type === "BOOL") {
      this.advance();
      return { type: "literal", value: tok.value === "true" };
    }

    if (tok.type === "IDENT") {
      this.advance();
      return { type: "field_access", field: tok.value };
    }

    throw new Error(`Unexpected token '${tok.value}' at position ${tok.pos}`);
  }

  private current(): Token {
    return this.tokens[this.pos] ?? { type: "EOF", value: "", pos: -1 };
  }

  private advance(): Token {
    const tok = this.tokens[this.pos];
    this.pos++;
    return tok;
  }

  private expect(type: TokenType): void {
    const tok = this.current();
    if (tok.type !== type) {
      throw new Error(`Expected ${type} but got '${tok.value}' at position ${tok.pos}`);
    }
    this.advance();
  }

  private peekIs(type: TokenType): boolean {
    return this.tokens[this.pos + 1]?.type === type;
  }
}

interface RenderResult {
  sql: string;
  params: unknown[];
  needsTagJoin: boolean;
}

function renderNode(node: ASTNode): RenderResult {
  switch (node.type) {
    case "binary": return renderBinary(node);
    case "unary": return renderUnary(node);
    case "field_access": return renderFieldAccess(node);
    case "literal": return renderLiteral(node);
    case "in": return renderIn(node);
    case "contains": return renderContains(node);
    case "starts_with": return renderStartsWith(node);
    case "ends_with": return renderEndsWith(node);
  }
}

function renderBinary(node: { op: string; left: ASTNode; right: ASTNode }): RenderResult {
  if (node.op === "&&" || node.op === "||") {
    const left = renderNode(node.left);
    const right = renderNode(node.right);
    return {
      sql: `(${left.sql}) ${node.op === "&&" ? "AND" : "OR"} (${right.sql})`,
      params: [...left.params, ...right.params],
      needsTagJoin: left.needsTagJoin || right.needsTagJoin
    };
  }

  const left = renderNode(node.left);
  const right = renderNode(node.right);
  const sqlOp = node.op === "==" ? "=" : node.op === "!=" ? "!=" : node.op;

  if (left.sql.includes("json_each") || right.sql.includes("json_each")) {
    return {
      sql: `${left.sql} ${sqlOp} ${right.sql}`,
      params: [...left.params, ...right.params],
      needsTagJoin: true
    };
  }

  return {
    sql: `${left.sql} ${sqlOp} ${right.sql}`,
    params: [...left.params, ...right.params],
    needsTagJoin: false
  };
}

function renderUnary(node: { op: string; operand: ASTNode }): RenderResult {
  const operand = renderNode(node.operand);
  return {
    sql: `NOT (${operand.sql})`,
    params: operand.params,
    needsTagJoin: operand.needsTagJoin
  };
}

function renderFieldAccess(node: { field: string; sub?: string }): RenderResult {
  const key = node.sub ? `${node.field}.${node.sub}` : node.field;
  const mapping = FIELD_MAP[key];
  if (!mapping) throw new Error(`Unknown field: ${key}`);

  if (mapping.type === "boolean") {
    return { sql: `${mapping.column} = 1`, params: [], needsTagJoin: false };
  }

  return { sql: mapping.column, params: [], needsTagJoin: mapping.type === "json_tag" };
}

function renderLiteral(node: { value: string | number | boolean }): RenderResult {
  if (typeof node.value === "boolean") {
    return { sql: node.value ? "1" : "0", params: [], needsTagJoin: false };
  }
  if (typeof node.value === "number") {
    return { sql: "?", params: [node.value], needsTagJoin: false };
  }
  return { sql: "?", params: [node.value], needsTagJoin: false };
}

function renderIn(node: { field: ASTNode; values: ASTNode[] }): RenderResult {
  const field = renderNode(node.field);
  const values = node.values.map(renderNode);
  const allParams = [...field.params, ...values.flatMap(v => v.params)];
  const placeholders = values.map(() => "?").join(", ");

  if (field.needsTagJoin) {
    return {
      sql: `${field.sql} IN (${placeholders})`,
      params: allParams,
      needsTagJoin: true
    };
  }

  return {
    sql: `${field.sql} IN (${placeholders})`,
    params: allParams,
    needsTagJoin: false
  };
}

function renderContains(node: { field: ASTNode; value: ASTNode }): RenderResult {
  const field = renderNode(node.field);
  const value = renderNode(node.value);
  return {
    sql: `${field.sql} LIKE '%' || ? || '%'`,
    params: [...field.params, ...value.params],
    needsTagJoin: field.needsTagJoin
  };
}

function renderStartsWith(node: { field: ASTNode; value: ASTNode }): RenderResult {
  const field = renderNode(node.field);
  const value = renderNode(node.value);
  return {
    sql: `${field.sql} LIKE ? || '%'`,
    params: [...field.params, ...value.params],
    needsTagJoin: field.needsTagJoin
  };
}

function renderEndsWith(node: { field: ASTNode; value: ASTNode }): RenderResult {
  const field = renderNode(node.field);
  const value = renderNode(node.value);
  return {
    sql: `${field.sql} LIKE '%' || ?`,
    params: [...field.params, ...value.params],
    needsTagJoin: field.needsTagJoin
  };
}

export interface FilterResult {
  sql: string;
  params: unknown[];
  needsTagJoin: boolean;
}

export function parseFilter(expression: string): FilterResult | null {
  const trimmed = expression.trim();
  if (!trimmed) return null;

  try {
    const lexer = new Lexer(trimmed);
    const tokens = lexer.tokenize();
    const parser = new Parser(tokens);
    const ast = parser.parse();
    const result = renderNode(ast);
    return result;
  } catch {
    return null;
  }
}
