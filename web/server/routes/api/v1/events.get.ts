// GET /api/v1/events — the host's SSE lifecycle stream, proxied with the body left STREAMING.
// Why a route of its own rather than the `/api/**` catch-all: see server/util/sseProxy.ts.
import { defineEventHandler } from "h3";
import { proxySse } from "../../../util/sseProxy";

export default defineEventHandler((event) => proxySse(event, "events"));
