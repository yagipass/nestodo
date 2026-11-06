import { defineHandler, defineHead, type InferProps } from "void";
import { statsFor } from "../src/stats-service";

export type Props = InferProps<typeof loader>;

export const loader = defineHandler((c) => statsFor(c));

export const head = defineHead(() => ({ title: "Stats" }));
