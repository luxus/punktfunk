import {
	MonitorPlay,
	RefreshCw,
	Users,
	Volume2,
	VolumeX,
	ZapOff,
} from "lucide-react";
import type { FC } from "react";
import type { SessionRow } from "@/api/gen/model/sessionRow";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "@/components/ui/select";
import { m } from "@/paraglide/messages";
import { levelLabel } from "@/sections/Pairing/access";

/**
 * Every live session, one row each — the host admits several at once and the card below this
 * one only ever showed the first of them.
 *
 * The actions are per row and reach exactly that session: stop, keyframe, mute, access level.
 * A compat-plane row carries no id (the host has no per-session handle for it), so its actions
 * are off and the card header's host-wide Stop is what ends it.
 *
 * Two columns are deliberately absent: which pads the session holds (#1094) and who owns the
 * audio device (#1093). Both are other issues and neither has a field on `SessionRow` yet.
 */
export const SessionList: FC<{
	sessions: SessionRow[];
	onStop: (row: SessionRow) => void;
	onIdr: (row: SessionRow) => void;
	onMute: (row: SessionRow, muted: boolean) => void;
	onAccess: (row: SessionRow, level: string) => void;
	busy: boolean;
}> = ({ sessions, onStop, onIdr, onMute, onAccess, busy }) => {
	if (sessions.length === 0) return null;
	return (
		<Card>
			<CardHeader>
				<CardTitle className="flex items-center gap-2">
					<Users className="size-4" />
					{m.sessions_title()}
				</CardTitle>
			</CardHeader>
			<CardContent className="flex flex-col gap-4">
				{sessions.map((s, i) => (
					<Row
						key={`${s.plane}:${s.id ?? "compat"}:${i}`}
						row={s}
						onStop={() => onStop(s)}
						onIdr={() => onIdr(s)}
						onMute={() => onMute(s, !s.muted)}
						onAccess={(level) => onAccess(s, level)}
						busy={busy}
					/>
				))}
			</CardContent>
		</Card>
	);
};

const Row: FC<{
	row: SessionRow;
	onStop: () => void;
	onIdr: () => void;
	onMute: () => void;
	onAccess: (level: string) => void;
	busy: boolean;
}> = ({ row, onStop, onIdr, onMute, onAccess, busy }) => {
	// No id means the compat plane: the host holds no per-session handle for it, so every
	// action here would silently become host-wide. Off is honest; the card below still stops it.
	const perSession = row.id != null;
	const facts = [
		row.mode,
		row.join ? m.sessions_joined() : m.sessions_own_display(),
		m.sessions_uptime({ time: formatUptime(row.uptime_s) }),
		row.plane === "gamestream" ? "GameStream" : undefined,
	].filter(Boolean);
	return (
		<div className="flex flex-col gap-3 border-b pb-4 last:border-0 last:pb-0 sm:flex-row sm:items-center">
			<div className="min-w-0 flex-1">
				<div className="flex flex-wrap items-center gap-2">
					<MonitorPlay className="size-4 shrink-0 text-muted-foreground" />
					<span className="truncate font-medium">
						{row.client_name || row.client}
					</span>
					{row.muted && <Badge variant="secondary">{m.sessions_muted()}</Badge>}
				</div>
				<p className="mt-0.5 truncate text-xs text-muted-foreground">
					{facts.join(" · ")}
				</p>
			</div>
			<div className="flex flex-wrap items-center gap-2">
				{/* Ungoverned on the compat plane — a select there would promise enforcement
				    the GameStream protocol has no way to carry. */}
				{row.access_level && perSession && (
					<Select
						value={row.access_level}
						onValueChange={onAccess}
						disabled={busy}
					>
						<SelectTrigger
							className="h-8 w-40"
							aria-label={m.access_level_label()}
						>
							<SelectValue />
						</SelectTrigger>
						<SelectContent>
							<SelectItem value="full">{m.access_level_full()}</SelectItem>
							<SelectItem value="controller">
								{m.access_level_controller()}
							</SelectItem>
							<SelectItem value="view">{m.access_level_view()}</SelectItem>
							{/* Only while the live mask is one the three presets cannot name. */}
							{row.access_level === "custom" && (
								<SelectItem value="custom" disabled>
									{levelLabel("custom")}
								</SelectItem>
							)}
						</SelectContent>
					</Select>
				)}
				<Button
					variant="outline"
					size="sm"
					disabled={!perSession || busy}
					onClick={onMute}
				>
					{row.muted ? (
						<VolumeX className="size-3.5" />
					) : (
						<Volume2 className="size-3.5" />
					)}
					{row.muted ? m.action_unmute() : m.action_mute()}
				</Button>
				<Button
					variant="outline"
					size="sm"
					disabled={!perSession || busy}
					onClick={onIdr}
				>
					<RefreshCw className="size-3.5" />
					{m.action_request_idr()}
				</Button>
				<Button
					variant="destructive"
					size="sm"
					disabled={!perSession || busy}
					onClick={onStop}
				>
					<ZapOff className="size-3.5" />
					{m.action_stop_session()}
				</Button>
			</div>
		</div>
	);
};

/** `h:mm` past an hour, else `m:ss` — a session's age reads as a duration, not seconds.
 * Shared with `LastSessionCard`, so a finished session reads the same as a live one. */
export function formatUptime(seconds: number): string {
	const s = Math.max(0, Math.floor(seconds));
	const mm = String(Math.floor((s % 3600) / 60)).padStart(2, "0");
	if (s >= 3600) return `${Math.floor(s / 3600)}:${mm}`;
	return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}
