// One controller drawing, lit from one `PadFrame`.
//
// The only renderer: every family is a row in `SHAPES`, so a new controller shape is
// coordinates, not another component. Triggers, shoulders, sticks and the D-pad are drawn
// here because they sit in the same place on all four.

import type { FC } from "react";
import type { PadFrame } from "@/api/gen/model/padFrame";
import { BIT, familyOf, SHAPES } from "./pads";

/** How far the dot travels from the well's centre at full deflection. */
const STICK_TRAVEL = 11;
const STICK_WELL = 15;

const lit = (buttons: number, bit: number) => (buttons & bit) !== 0;

/** Lit face, dim otherwise. Fill and stroke both move, so a press reads at a glance. */
const tone = (on: boolean) => ({
	fill: on ? "var(--primary)" : "var(--muted)",
	stroke: on ? "var(--primary)" : "var(--border)",
});

/** A trigger's 0–255 pull as a filled bar. */
const Trigger: FC<{ x: number; value: number; label: string }> = ({
	x,
	value,
	label,
}) => (
	<g>
		<rect
			x={x}
			y={6}
			width={56}
			height={13}
			rx={4}
			fill="var(--muted)"
			stroke="var(--border)"
		/>
		<rect
			x={x}
			y={6}
			width={(56 * value) / 255}
			height={13}
			rx={4}
			fill="var(--primary)"
		/>
		<text
			x={x + 28}
			y={16}
			textAnchor="middle"
			fontSize={8}
			fill="var(--muted-foreground)"
		>
			{label} {value}
		</text>
	</g>
);

/** Four segments, each lit by its own wire bit. */
const Dpad: FC<{ cx: number; cy: number; buttons: number }> = ({
	cx,
	cy,
	buttons,
}) => {
	const arm = 8;
	const seg = (bit: number, dx: number, dy: number, w: number, h: number) => (
		<rect
			key={bit}
			x={cx + dx}
			y={cy + dy}
			width={w}
			height={h}
			rx={2}
			{...tone(lit(buttons, bit))}
		/>
	);
	return (
		<g>
			{seg(BIT.DPAD_UP, -4, -arm - 8, 8, arm)}
			{seg(BIT.DPAD_DOWN, -4, 8, 8, arm)}
			{seg(BIT.DPAD_LEFT, -arm - 8, -4, arm, 8)}
			{seg(BIT.DPAD_RIGHT, 8, -4, arm, 8)}
		</g>
	);
};

/** A stick: its well, and a dot where the host has it. `+y = up` on the wire, so y inverts. */
const Stick: FC<{
	cx: number;
	cy: number;
	x: number;
	y: number;
	clicked: boolean;
}> = ({ cx, cy, x, y, clicked }) => (
	<g>
		<circle
			cx={cx}
			cy={cy}
			r={STICK_WELL}
			fill="var(--muted)"
			stroke={clicked ? "var(--primary)" : "var(--border)"}
			strokeWidth={clicked ? 2 : 1}
		/>
		<circle
			cx={cx + (x / 32767) * STICK_TRAVEL}
			cy={cy - (y / 32767) * STICK_TRAVEL}
			r={5}
			fill="var(--primary)"
		/>
	</g>
);

export const PadDiagram: FC<{ frame: PadFrame }> = ({ frame }) => {
	const shape = SHAPES[familyOf(frame.device)];
	const b = frame.buttons;
	return (
		<svg
			viewBox="0 0 260 160"
			role="img"
			aria-label={`Pad ${frame.pad}`}
			className="w-full max-w-sm"
		>
			<path d={shape.body} fill="var(--card)" stroke="var(--border)" />
			<Trigger x={34} value={frame.left_trigger} label="LT" />
			<Trigger x={170} value={frame.right_trigger} label="RT" />
			<rect
				x={34}
				y={22}
				width={56}
				height={9}
				rx={3}
				{...tone(lit(b, BIT.LB))}
			/>
			<rect
				x={170}
				y={22}
				width={56}
				height={9}
				rx={3}
				{...tone(lit(b, BIT.RB))}
			/>
			{shape.plates.map((pl) => (
				<rect
					key={`${pl.x},${pl.y}`}
					x={pl.x}
					y={pl.y}
					width={pl.w}
					height={pl.h}
					rx={4}
					{...tone(pl.bit !== undefined && lit(b, pl.bit))}
				/>
			))}
			<Dpad cx={shape.dpad.cx} cy={shape.dpad.cy} buttons={b} />
			<Stick
				cx={shape.sticks[0].cx}
				cy={shape.sticks[0].cy}
				x={frame.ls_x}
				y={frame.ls_y}
				clicked={lit(b, shape.sticks[0].bit)}
			/>
			<Stick
				cx={shape.sticks[1].cx}
				cy={shape.sticks[1].cy}
				x={frame.rs_x}
				y={frame.rs_y}
				clicked={lit(b, shape.sticks[1].bit)}
			/>
			{shape.buttons.map((btn) => (
				<g key={btn.bit}>
					<circle
						cx={btn.cx}
						cy={btn.cy}
						r={btn.r ?? 9}
						{...tone(lit(b, btn.bit))}
					/>
					{btn.label && (
						<text
							x={btn.cx}
							y={btn.cy + 3}
							textAnchor="middle"
							fontSize={9}
							fill={
								lit(b, btn.bit)
									? "var(--primary-foreground)"
									: "var(--muted-foreground)"
							}
						>
							{btn.label}
						</text>
					)}
				</g>
			))}
		</svg>
	);
};
