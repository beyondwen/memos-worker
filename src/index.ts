import type { Env } from "./types";
import { HttpError, json } from "./utils";
import { route } from "./router";

export { SSEHub } from "./sse";
export { hashPassword, verifyPassword } from "./auth";
export { buildMemoPayload } from "./services/memo";
export { sanitizeFilename } from "./utils";

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    try {
      return await route(request, env);
    } catch (error) {
      if (error instanceof HttpError) {
        return json({ error: error.message }, error.status);
      }
      console.error(error);
      return json({ error: "Internal server error" }, 500);
    }
  }
};
