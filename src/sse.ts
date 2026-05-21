import type { Env, SseEvent, Role } from "./types";
import { verifyJwt } from "./auth";
import { json, encoder } from "./utils";

export class SSEHub {
  private sessions = new Map<string, {
    userId: number;
    role: Role;
    writer: WritableStreamDefaultWriter<Uint8Array>;
  }>();

  constructor(private state: DurableObjectState, private env: Env) {}

  async fetch(request: Request): Promise<Response> {
    const url = new URL(request.url);

    if (request.method === "POST" && url.pathname === "/broadcast") {
      const event = await request.json<SseEvent>();
      await this.broadcast(event);
      return json({ ok: true });
    }

    if (request.method !== "GET" || url.pathname !== "/connect") {
      return json({ error: "Not found" }, 404);
    }

    const token = url.searchParams.get("token");
    if (!token) return json({ error: "Missing token" }, 401);

    const claims = await verifyJwt(token, this.env.SERVER_SECRET);
    if (!claims || claims.type !== "sse") return json({ error: "Invalid token" }, 401);

    const sessionId = crypto.randomUUID();
    const stream = new TransformStream<Uint8Array, Uint8Array>();
    const writer = stream.writable.getWriter();

    this.sessions.set(sessionId, {
      userId: Number(claims.sub),
      role: claims.role,
      writer
    });

    const heartbeat = setInterval(() => {
      writer.write(encoder.encode(`: heartbeat\n\n`)).catch(() => {
        clearInterval(heartbeat);
        this.sessions.delete(sessionId);
      });
    }, 30000);

    writer.write(encoder.encode(`event: ready\ndata: {}\n\n`)).catch(() => {
      clearInterval(heartbeat);
      this.sessions.delete(sessionId);
    });

    request.signal.addEventListener("abort", () => {
      clearInterval(heartbeat);
      this.sessions.delete(sessionId);
      writer.close().catch(() => undefined);
    });

    return new Response(stream.readable, {
      headers: {
        "Content-Type": "text/event-stream; charset=utf-8",
        "Cache-Control": "no-cache, no-transform",
        "X-Accel-Buffering": "no"
      }
    });
  }

  private async broadcast(event: SseEvent): Promise<void> {
    const chunk = encoder.encode(
      `id: ${event.id}\nevent: ${event.type}\ndata: ${JSON.stringify(event)}\n\n`
    );

    const writes: Promise<unknown>[] = [];
    for (const [sessionId, session] of this.sessions) {
      if (!canReceiveEvent(event, session.userId, session.role)) continue;
      writes.push(session.writer.write(chunk).catch(() => {
        this.sessions.delete(sessionId);
      }));
    }
    await Promise.all(writes);
  }
}

export function canReceiveEvent(event: SseEvent, userId: number, role: Role): boolean {
  if (role === "ADMIN") return true;
  if (event.visibility !== "PRIVATE") return true;
  return event.creatorId === userId;
}
