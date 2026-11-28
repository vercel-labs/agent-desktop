import { renderLabsOg } from "./labs-og";

export async function GET() {
  return renderLabsOg({
    product: "agent-desktop",
    title: "Desktop automation for AI",
  });
}
