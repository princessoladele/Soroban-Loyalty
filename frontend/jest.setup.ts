import "@testing-library/jest-dom";
import { configureAxe } from "jest-axe";

// Default axe config: WCAG 2.1 AA
configureAxe({ globalOptions: { rules: [{ id: "color-contrast", enabled: true }] } });
