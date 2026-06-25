export function formatDate(value: string) {
  return new Date(value).toLocaleString();
}

export function diffColor(value: string) {
  return value === "added" ? "green" : value === "removed" ? "red" : "gold";
}

export function riskColor(value: string) {
  return value === "low" ? "green" : value === "medium" ? "gold" : "red";
}
