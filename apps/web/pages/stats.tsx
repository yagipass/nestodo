import { useEffect, useState, type PointerEvent } from "react";
import { useRouter } from "@void/react";
import { AppHeader } from "./layout";
import type { Props } from "./stats.server";
import { RANGES, heatLevel, summarize, ticks, total, type Day, type Range } from "../src/stats";

const utc = { timeZone: "UTC" } as const;
const shortDate = new Intl.DateTimeFormat("en-US", { ...utc, month: "short", day: "numeric" });
const longDate = new Intl.DateTimeFormat("en-US", { ...utc, weekday: "short", month: "short", day: "numeric" });
const weekday = new Intl.DateTimeFormat("en-US", { ...utc, weekday: "short" });
const monthName = new Intl.DateTimeFormat("en-US", { ...utc, month: "short" });
const WEEKDAY_LABELS = ["", "Mon", "", "Wed", "", "Fri", ""];

function format(formatter: Intl.DateTimeFormat, date: string) {
  return formatter.format(Date.parse(date));
}

function signed(value: number) {
  return value > 0 ? `+${value}` : value < 0 ? `−${-value}` : "±0";
}

function round(value: number) {
  return Math.round(value * 100) / 100;
}

function edge(percent: number) {
  return percent < 10 ? "translateX(0)" : percent > 90 ? "translateX(-100%)" : "translateX(-50%)";
}

function useHover() {
  const [hover, setHover] = useState<number | null>(null);
  const leave = (event: PointerEvent) => {
    if (event.pointerType === "mouse") setHover(null);
  };
  return [hover, setHover, leave] as const;
}

function Gridlines({ scale }: { scale: number[] }) {
  const top = scale[scale.length - 1];
  return scale.map((value) => (
    <span key={value} className="gridline" style={{ bottom: `${round((value / top) * 100)}%` }}>
      <span className="tick">{value}</span>
    </span>
  ));
}

function AxisLabels({ days, at }: { days: Day[]; at: (index: number) => number }) {
  const every = days.length <= 7 ? 1 : days.length <= 30 ? 7 : 14;
  const indexes: number[] = [];
  for (let index = days.length - 1; index >= 0; index -= every) indexes.unshift(index);
  return (
    <div className="axis" aria-hidden="true">
      {indexes.map((index) => (
        <span key={index} style={{ left: `${round(at(index))}%`, transform: edge(at(index)) }}>
          {format(days.length <= 7 ? weekday : shortDate, days[index].date)}
        </span>
      ))}
    </div>
  );
}

function Tip({ left, bottom, value, detail }: { left: number; bottom: number; value: string; detail: string }) {
  return (
    <div className="tip" style={{ left: `${round(left)}%`, bottom: `calc(${round(bottom)}% + 10px)`, transform: edge(left) }}>
      <strong>{value}</strong>
      <span>{detail}</span>
    </div>
  );
}

function Calendar({ days }: { days: Day[] }) {
  const [hover, setHover, leave] = useHover();
  const blanks = new Date(Date.parse(days[0].date)).getUTCDay();
  const weeks = Math.ceil((blanks + days.length) / 7);
  const activity = (day: Day) => day.added + day.completed;
  const busiest = Math.max(...days.map(activity));
  const added = total(days, "added");
  const completed = total(days, "completed");
  const place = (index: number) => ({ gridColumn: Math.floor((blanks + index) / 7) + 2, gridRow: ((blanks + index) % 7) + 2 });
  const months = days.flatMap((day, index) => (day.date.endsWith("-01") ? [{ date: day.date, column: place(index).gridColumn }] : []));
  const hovered = hover === null ? null : days[hover];

  return (
    <section className="card">
      <h2>Past 26 weeks</h2>
      <p className="card-note">Todos added and completed each day</p>
      <div
        className="calendar"
        role="img"
        aria-label="Calendar of todos added and completed each day over the past 26 weeks"
        style={{ gridTemplateColumns: `28px repeat(${weeks}, minmax(0, 1fr))` }}
        onPointerLeave={leave}
      >
        {months.map((month) => (
          <span key={month.date} className="calendar-month" style={{ gridColumn: `${month.column} / span ${Math.min(3, weeks + 2 - month.column)}` }}>
            {format(monthName, month.date)}
          </span>
        ))}
        {WEEKDAY_LABELS.map((label, index) => (
          <span key={index} className="calendar-weekday" style={{ gridRow: index + 2 }}>
            {label}
          </span>
        ))}
        {days.map((day, index) => (
          <span
            key={day.date}
            className={index === hover ? "cell active" : "cell"}
            data-level={heatLevel(activity(day), busiest)}
            style={place(index)}
            onPointerEnter={() => setHover(index)}
          />
        ))}
      </div>
      <div className="calendar-foot">
        <span aria-live="polite">
          {hovered
            ? `${hovered.added} added · ${hovered.completed} completed · ${format(longDate, hovered.date)}`
            : `${added} added · ${completed} completed in the past 26 weeks`}
        </span>
        <span className="legend" aria-hidden="true">
          Less
          {[0, 1, 2, 3, 4].map((level) => (
            <span key={level} className="cell" data-level={level} />
          ))}
          More
        </span>
      </div>
    </section>
  );
}

function MirroredGridlines({ scale }: { scale: number[] }) {
  const top = scale[scale.length - 1];
  return [...scale.slice(1).reverse().map((value) => -value), ...scale].map((value) => (
    <span key={value} className={value === 0 ? "gridline baseline" : "gridline"} style={{ bottom: `${round(50 + (value / top) * 50)}%` }}>
      <span className="tick">{Math.abs(value)}</span>
    </span>
  ));
}

function AddedCompletedChart({ days }: { days: Day[] }) {
  const [hover, setHover, leave] = useHover();
  const scale = ticks(Math.max(...days.map((day) => Math.max(day.added, day.completed))));
  const top = scale[scale.length - 1];
  const openScale = ticks(Math.max(...days.map((day) => day.open)));
  const openTop = openScale[openScale.length - 1];
  const at = (index: number) => ((index + 0.5) / days.length) * 100;
  const height = (count: number) => (count / top) * 100;
  const level = (day: Day) => (day.open / openTop) * 100;
  const line = `M${days.map((day, index) => `${round(at(index) * 10)},${round(140 - level(day) * 1.4)}`).join(" L")}`;
  const floor = `L${round(at(days.length - 1) * 10)},140 L${round(at(0) * 10)},140 Z`;
  const focus = hover ?? days.length - 1;
  const hovered = hover === null ? null : days[hover];
  const added = total(days, "added");
  const completed = total(days, "completed");
  const bar = (kind: "added" | "completed", index: number) => (index === hover ? `bar ${kind} active` : `bar ${kind}`);

  return (
    <section className="card">
      <div className="card-head">
        <div>
          <h2>Added and completed</h2>
          <p className="card-note">
            {added} added and {completed} completed in the last {days.length} days
          </p>
        </div>
        <span className="legend series" aria-hidden="true">
          <span className="legend-item">
            <span className="swatch added" />
            Added ↑
          </span>
          <span className="legend-item">
            <span className="swatch completed" />
            Completed ↓
          </span>
        </span>
      </div>
      <div className="plot mirrored" role="img" aria-label="Column chart of todos added per day above the line and completed per day below it">
        <MirroredGridlines scale={scale} />
        <div className={days.length > 30 ? "columns dense" : "columns"} onPointerLeave={leave}>
          {days.map((day, index) => (
            <div key={day.date} className="column pair" onPointerEnter={() => setHover(index)}>
              <span className="half">
                <span className={bar("added", index)} style={{ height: `${round(height(day.added))}%` }} />
              </span>
              <span className="half">
                <span className={bar("completed", index)} style={{ height: `${round(height(day.completed))}%` }} />
              </span>
            </div>
          ))}
        </div>
        {hover !== null && hovered && (
          <Tip
            left={at(hover)}
            bottom={50 + height(hovered.added) / 2}
            value={`${hovered.added} added · ${hovered.completed} completed`}
            detail={format(longDate, hovered.date)}
          />
        )}
      </div>
      <p className="plot-label">Open at the end of each day</p>
      <div className="plot short" role="img" aria-label="Line chart of open todos at the end of each day">
        <Gridlines scale={openScale} />
        <svg viewBox="0 0 1000 140" preserveAspectRatio="none" aria-hidden="true">
          <path className="area" d={`${line} ${floor}`} />
          <path className="line" d={line} />
        </svg>
        {hover !== null && <span className="crosshair" style={{ left: `${round(at(hover))}%` }} />}
        <span className="dot" style={{ left: `${round(at(focus))}%`, bottom: `${round(level(days[focus]))}%` }} />
        <span
          className="point-label"
          style={{ left: `${round(at(focus))}%`, bottom: `calc(${round(level(days[focus]))}% + 10px)`, transform: edge(at(focus)) }}
        >
          {days[focus].open} open
        </span>
        <div className="hits" onPointerLeave={leave}>
          {days.map((day, index) => (
            <span
              key={day.date}
              className="hit"
              style={{ left: `${round((index / days.length) * 100)}%`, width: `${round(100 / days.length)}%` }}
              onPointerEnter={() => setHover(index)}
            />
          ))}
        </div>
      </div>
      <AxisLabels days={days} at={at} />
      <details className="table-view">
        <summary>Show as table</summary>
        <div className="table-scroll">
          <table>
            <thead>
              <tr>
                <th scope="col">Day</th>
                <th scope="col">Added</th>
                <th scope="col">Completed</th>
                <th scope="col">Open</th>
              </tr>
            </thead>
            <tbody>
              {[...days].reverse().map((day) => (
                <tr key={day.date}>
                  <th scope="row">{format(longDate, day.date)}</th>
                  <td>{day.added}</td>
                  <td>{day.completed}</td>
                  <td>{day.open}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </details>
    </section>
  );
}

export default function StatsPage({ timeZone, days }: Props) {
  const router = useRouter();
  const [range, setRange] = useState<Range>(30);

  useEffect(() => {
    const local = Intl.DateTimeFormat().resolvedOptions().timeZone;
    if (local && local !== timeZone && router.query.get("tz") !== local) {
      void router.visit(`/stats?tz=${encodeURIComponent(local)}`, { replace: true });
    }
  }, [router, timeZone]);

  const shown = days.slice(-range);
  const totals = summarize(days, range);
  const period = `${range} days`;
  const tiles = [
    { label: "Completed", value: totals.completed, note: `${signed(totals.completed - totals.completedBefore)} vs previous ${period}` },
    { label: "Added", value: totals.added, note: `${signed(totals.added - totals.addedBefore)} vs previous ${period}` },
    { label: "Open now", value: totals.open, note: `${signed(totals.open - totals.openBefore)} since ${format(shortDate, totals.since)}` },
    { label: "Per day", value: (totals.completed / range).toFixed(1), note: "completed on average" },
  ];

  return (
    <>
      <AppHeader />
      <main className="stats">
        <h1>Stats</h1>
        <Calendar days={days} />
        <div className="ranges">
          {RANGES.map((value) => (
            <button key={value} type="button" className="pill" aria-pressed={value === range} onClick={() => setRange(value)}>
              {value} days
            </button>
          ))}
        </div>
        <dl className="tiles">
          {tiles.map((tile) => (
            <div key={tile.label} className="tile">
              <dt>{tile.label}</dt>
              <dd className="tile-value">{tile.value}</dd>
              <dd className="tile-note">{tile.note}</dd>
            </div>
          ))}
        </dl>
        <AddedCompletedChart key={range} days={shown} />
      </main>
    </>
  );
}
