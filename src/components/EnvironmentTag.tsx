import { Tag } from "antd";

export function EnvironmentTag({ value }: { value?: string }) {
  const colors: Record<string, string> = {
    Development: "green",
    Testing: "blue",
    Staging: "gold",
    Production: "red",
  };

  return <Tag color={colors[value || ""]}>{value || "Development"}</Tag>;
}
