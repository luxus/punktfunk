import { createFileRoute } from "@tanstack/react-router";
import { SectionControllers } from "@/sections/Controllers";

export const Route = createFileRoute("/controllers")({
	component: SectionControllers,
});
