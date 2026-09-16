// GET /api/v1/session/{id}/pads — the host's live pad feed, proxied with the body left
// STREAMING (server/util/sseProxy.ts says why a route of its own).
//
// The id must be digits before it is pasted into a URL the server dials.
import { createError, defineEventHandler, getRouterParam } from "h3";
import { proxySse } from "../../../../../util/sseProxy";

export default defineEventHandler((event) => {
	const id = getRouterParam(event, "id");
	if (!id || !/^\d+$/.test(id)) {
		throw createError({ statusCode: 400, statusMessage: "bad session id" });
	}
	return proxySse(event, `session/${id}/pads`);
});
