"use client";

import { useState } from "react";
import Link from "next/link";
import { Campaign } from "@/lib/api";

type SortKey = "id" | "reward_amount" | "total_claimed" | "expiration";
type SortDir = "asc" | "desc";

const PAGE_SIZE = 20;

interface Props {
  campaigns: Campaign[];
  onDeactivate?: (id: number) => Promise<void>;
  merchantPublicKey?: string;
}

export function CampaignTable({ campaigns, onDeactivate, merchantPublicKey }: Props) {
  const [search, setSearch] = useState("");
  const [sortKey, setSortKey] = useState<SortKey>("id");
  const [sortDir, setSortDir] = useState<SortDir>("desc");
  const [page, setPage] = useState(1);
  const [confirmId, setConfirmId] = useState<number | null>(null);
  const [deactivating, setDeactivating] = useState<number | null>(null);

  const now = Date.now() / 1000;

  const filtered = campaigns.filter((c) =>
    `campaign #${c.id} ${c.merchant}`.toLowerCase().includes(search.toLowerCase())
  );

  const sorted = [...filtered].sort((a, b) => {
    const mul = sortDir === "asc" ? 1 : -1;
    return (a[sortKey] - b[sortKey]) * mul;
  });

  const totalPages = Math.max(1, Math.ceil(sorted.length / PAGE_SIZE));
  const pageData = sorted.slice((page - 1) * PAGE_SIZE, page * PAGE_SIZE);

  const toggleSort = (key: SortKey) => {
    if (sortKey === key) setSortDir((d) => (d === "asc" ? "desc" : "asc"));
    else { setSortKey(key); setSortDir("asc"); }
    setPage(1);
  };

  const arrow = (key: SortKey) =>
    sortKey === key ? (sortDir === "asc" ? " ▲" : " ▼") : "";

  const handleDeactivate = async (id: number) => {
    if (!onDeactivate) return;
    setDeactivating(id);
    try { await onDeactivate(id); } finally {
      setDeactivating(null);
      setConfirmId(null);
    }
  };

  const getStatus = (c: Campaign) =>
    !c.active ? "Inactive" : now > c.expiration ? "Expired" : "Active";

  return (
    <div>
      <div style={{ marginBottom: 12 }}>
        <input
          type="search"
          placeholder="Search campaigns…"
          value={search}
          onChange={(e) => { setSearch(e.target.value); setPage(1); }}
          style={{
            background: "#0f1117", border: "1px solid #2d3148", borderRadius: 8,
            padding: "8px 12px", color: "#e2e8f0", fontSize: "0.875rem", width: "100%", maxWidth: 320,
          }}
        />
      </div>

      <div style={{ overflowX: "auto" }}>
        <table style={{ width: "100%", borderCollapse: "collapse", minWidth: 640 }}>
          <thead>
            <tr style={{ borderBottom: "1px solid #2d3148", color: "#64748b", fontSize: "0.8rem" }}>
              {(
                [
                  ["id", "ID"],
                  ["reward_amount", "Reward"],
                  ["total_claimed", "Claims"],
                  ["expiration", "Expiry"],
                ] as [SortKey, string][]
              ).map(([key, label]) => (
                <th
                  key={key}
                  onClick={() => toggleSort(key)}
                  style={{ textAlign: "left", padding: "8px 12px", cursor: "pointer", userSelect: "none", whiteSpace: "nowrap" }}
                >
                  {label}{arrow(key)}
                </th>
              ))}
              <th style={{ textAlign: "left", padding: "8px 12px" }}>Status</th>
              {onDeactivate && <th style={{ padding: "8px 12px" }}>Actions</th>}
            </tr>
          </thead>
          <tbody>
            {pageData.length === 0 ? (
              <tr>
                <td colSpan={6} style={{ textAlign: "center", padding: 32, color: "#64748b" }}>
                  No campaigns found.
                </td>
              </tr>
            ) : pageData.map((c) => {
              const status = getStatus(c);
              return (
                <tr key={c.id} style={{ borderBottom: "1px solid #1a1d27" }}>
                  <td style={{ padding: "10px 12px" }}>
                    <Link href={`/campaigns/${c.id}`} style={{ color: "var(--accent)", textDecoration: "none", fontWeight: 600 }}>
                      #{c.id}
                    </Link>
                  </td>
                  <td style={{ padding: "10px 12px" }}>{c.reward_amount.toLocaleString()} LYT</td>
                  <td style={{ padding: "10px 12px" }}>{c.total_claimed}</td>
                  <td style={{ padding: "10px 12px", fontSize: "0.8rem" }}>
                    {new Date(c.expiration * 1000).toLocaleDateString()}
                  </td>
                  <td style={{ padding: "10px 12px" }}>
                    <span className="badge" data-status={status.toLowerCase()}>{status}</span>
                  </td>
                  {onDeactivate && (
                    <td style={{ padding: "10px 12px" }}>
                      {confirmId === c.id ? (
                        <span style={{ display: "flex", gap: 6 }}>
                          <button
                            className="btn btn-outline"
                            style={{ padding: "4px 10px", fontSize: "0.75rem" }}
                            onClick={() => setConfirmId(null)}
                          >
                            Cancel
                          </button>
                          <button
                            className="btn btn-primary"
                            style={{ padding: "4px 10px", fontSize: "0.75rem", background: "#dc2626" }}
                            disabled={deactivating === c.id}
                            onClick={() => handleDeactivate(c.id)}
                          >
                            {deactivating === c.id ? "…" : "Confirm"}
                          </button>
                        </span>
                      ) : (
                        <button
                          className="btn btn-outline"
                          style={{ padding: "4px 10px", fontSize: "0.75rem" }}
                          disabled={!c.active}
                          onClick={() => setConfirmId(c.id)}
                        >
                          Deactivate
                        </button>
                      )}
                    </td>
                  )}
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>

      {totalPages > 1 && (
        <div style={{ display: "flex", gap: 8, alignItems: "center", marginTop: 12, fontSize: "0.875rem" }}>
          <button className="btn btn-outline" style={{ padding: "4px 12px" }} disabled={page === 1} onClick={() => setPage((p) => p - 1)}>
            ‹ Prev
          </button>
          <span style={{ color: "#64748b" }}>Page {page} / {totalPages}</span>
          <button className="btn btn-outline" style={{ padding: "4px 12px" }} disabled={page === totalPages} onClick={() => setPage((p) => p + 1)}>
            Next ›
          </button>
        </div>
      )}
    </div>
  );
}
