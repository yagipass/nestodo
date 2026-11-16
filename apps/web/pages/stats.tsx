import { useEffect } from "react";
import { useRouter } from "@void/react";
import { AppHeader } from "./layout";
import type { Props } from "./stats.server";
import { StatsView } from "../src/stats-view";

export default function StatsPage({ timeZone, days }: Props) {
  const router = useRouter();

  useEffect(() => {
    const local = Intl.DateTimeFormat().resolvedOptions().timeZone;
    if (local && local !== timeZone && router.query.get("tz") !== local) {
      void router.visit(`/stats?tz=${encodeURIComponent(local)}`, { replace: true });
    }
  }, [router, timeZone]);

  return (
    <>
      <AppHeader />
      <main className="stats">
        <h1>Stats</h1>
        <StatsView days={days} />
      </main>
    </>
  );
}
