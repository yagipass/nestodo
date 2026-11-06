import { defineHandler } from "void";
import { statsFor } from "../../../src/stats-service";

export const GET = defineHandler((c) => statsFor(c));
