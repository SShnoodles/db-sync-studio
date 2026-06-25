import type { DbConnection } from "../types";

export function connectionOptionLabel(connection: DbConnection) {
  const environment = connection.environment?.trim();
  return [connection.name, connection.database, environment].filter(Boolean).join(" · ");
}
