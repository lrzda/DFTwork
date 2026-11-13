/**
 * 构建与放行面板。
 *
 * 三段对应 ADR-004：展示计划（diff）→ 用户放行 → 回执与回滚。
 * 这里**不发起任何写操作**，只把用户点「放行」翻译成一次 `api.apply`。
 */
import { useMemo } from "react";
import {
  Alert,
  Button,
  Card,
  Collapse,
  Descriptions,
  Empty,
  Popconfirm,
  Select,
  Space,
  Table,
  Tag,
  Typography,
} from "antd";
import type {
  BuildItem,
  BuildPlan,
  BuildReceipt,
  ReceiptRecord,
  RecoveryNotice,
  RollbackReport,
} from "@dftwork/shared";

const { Text, Paragraph } = Typography;

type DiffLine = { type: "same" | "add" | "del"; text: string };

/**
 * 行级 diff（LCS）。
 *
 * INCAR/POSCAR 都是几十到几百行，O(n·m) 完全够用，
 * 不值得为此引入一个 diff 库。
 */
function lineDiff(oldText: string, newText: string): DiffLine[] {
  const a = oldText.split("\n");
  const b = newText.split("\n");
  const n = a.length;
  const m = b.length;
  const dp: number[][] = Array.from({ length: n + 1 }, () => new Array<number>(m + 1).fill(0));

  for (let i = n - 1; i >= 0; i -= 1) {
    for (let j = m - 1; j >= 0; j -= 1) {
      dp[i][j] = a[i] === b[j] ? dp[i + 1][j + 1] + 1 : Math.max(dp[i + 1][j], dp[i][j + 1]);
    }
  }

  const out: DiffLine[] = [];
  let i = 0;
  let j = 0;
  while (i < n && j < m) {
    if (a[i] === b[j]) {
      out.push({ type: "same", text: a[i] });
      i += 1;
      j += 1;
    } else if (dp[i + 1][j] >= dp[i][j + 1]) {
      out.push({ type: "del", text: a[i] });
      i += 1;
    } else {
      out.push({ type: "add", text: b[j] });
      j += 1;
    }
  }
  while (i < n) {
    out.push({ type: "del", text: a[i] });
    i += 1;
  }
  while (j < m) {
    out.push({ type: "add", text: b[j] });
    j += 1;
  }
  return out;
}

function DiffView({ item }: { item: BuildItem }) {
  const lines = useMemo(() => {
    if (item.previous_content === null) {
      // 新建：整份都是新增
      return item.content.split("\n").map((t) => ({ type: "add" as const, text: t }));
    }
    return lineDiff(item.previous_content, item.content);
  }, [item]);

  const changed = lines.filter((l) => l.type !== "same").length;

  return (
    <div>
      <Paragraph type="secondary" style={{ fontSize: 12, marginBottom: 6 }}>
        {item.reason} · {changed === 0 ? "内容无变化" : `${changed} 行有差异`}
      </Paragraph>
      <pre
        style={{
          margin: 0,
          maxHeight: 260,
          overflow: "auto",
          fontSize: 12,
          lineHeight: 1.55,
          background: "rgba(127,127,127,0.08)",
          borderRadius: 4,
          padding: "8px 10px",
        }}
      >
        {lines.map((l, idx) => (
          <div
            key={idx}
            style={{
              background:
                l.type === "add"
                  ? "rgba(82,196,26,0.16)"
                  : l.type === "del"
                    ? "rgba(255,77,79,0.16)"
                    : undefined,
              color: l.type === "del" ? "#ffa39e" : undefined,
            }}
          >
            <span style={{ opacity: 0.55, userSelect: "none", marginRight: 8 }}>
              {l.type === "add" ? "+" : l.type === "del" ? "−" : " "}
            </span>
            {l.text || " "}
          </div>
        ))}
      </pre>
    </div>
  );
}

const ACTION_TAG = {
  create: { color: "green", label: "新建" },
  overwrite: { color: "orange", label: "覆盖" },
  skip: { color: "default", label: "不动" },
} as const;

function receiptId(receipt: BuildReceipt): string {
  return `${receipt.plan_hash}:${receipt.finished_at}`;
}

export function BuildPanel({
  plan,
  receipt,
  receipts,
  recovery,
  backupBytes,
  rollbackReport,
  busy,
  onApply,
  onRollback,
  onDiscard,
  onRecover,
  onPrune,
  onSelectReceipt,
}: {
  plan: BuildPlan | null;
  receipt: BuildReceipt | null;
  receipts: ReceiptRecord[];
  recovery: RecoveryNotice | null;
  backupBytes: number;
  rollbackReport: RollbackReport | null;
  busy: boolean;
  onApply: () => void;
  onRollback: () => void;
  onDiscard: () => void;
  onRecover: () => void;
  onPrune: () => void;
  onSelectReceipt: (record: ReceiptRecord) => void;
}) {
  const selectedRecord = receipt
    ? receipts.find((record) => receiptId(record.receipt) === receiptId(receipt))
    : undefined;
  const alreadyRolledBack = Boolean(selectedRecord?.rolled_back || rollbackReport);
  const willWrite = plan?.items.filter((item) => item.action !== "skip") ?? [];
  const overwrites = plan?.items.filter((item) => item.action === "overwrite").length ?? 0;
  const allSkip = willWrite.length === 0;

  return (
    <Space direction="vertical" size={10} style={{ width: "100%" }}>
      {recovery && (
        <Alert
          type={recovery.committed ? "warning" : "error"}
          showIcon
          message={recovery.committed ? "回执已提交，尚需完成清理" : "检测到中断的写入"}
          description={
            recovery.committed
              ? `计划 ${recovery.plan_hash.slice(0, 12)} 的文件与回执已保留；恢复操作不会回滚该批次。`
              : `计划 ${recovery.plan_hash.slice(0, 12)} 可能只写入了一部分。恢复会先校验后像，再还原覆盖项并删除本次新建项；后续人工修改会阻止自动恢复。`
          }
          action={
            <Button type="primary" danger={!recovery.committed} loading={busy} onClick={onRecover}>
              {recovery.committed ? "完成恢复" : "恢复原状态"}
            </Button>
          }
        />
      )}

      {receipt ? (
        <>
          <Alert
            type={alreadyRolledBack ? "warning" : "success"}
            showIcon
            message={alreadyRolledBack ? "历史回执：已回滚" : "已写入"}
            description={`写入 ${receipt.written.length} 个、跳过 ${receipt.skipped.length} 个${
              receipt.backup_dir ? `，备份在 ${receipt.backup_dir}` : "（无覆盖项，未产生备份）"
            }`}
          />
          <Descriptions size="small" column={1} bordered>
            <Descriptions.Item label="计划指纹">
              <Text code>{receipt.plan_hash}</Text>
            </Descriptions.Item>
            <Descriptions.Item label="写入">
              {receipt.written.length === 0 ? <Text type="secondary">无</Text> : receipt.written.join("、")}
            </Descriptions.Item>
            <Descriptions.Item label="跳过">
              {receipt.skipped.length === 0 ? <Text type="secondary">无</Text> : receipt.skipped.join("、")}
            </Descriptions.Item>
            <Descriptions.Item label="完成时间">{receipt.finished_at}</Descriptions.Item>
          </Descriptions>
          {selectedRecord?.rollback_error && (
            <Alert type="warning" showIcon message={selectedRecord.rollback_error} />
          )}
          <Space>
            <Popconfirm
              title="回滚这次写入？"
              description="覆盖过的文件会还原成原内容，新建的会被删除。备份目录保留至保留策略清理。"
              okText="回滚"
              cancelText="取消"
              onConfirm={onRollback}
              disabled={alreadyRolledBack || Boolean(selectedRecord?.rollback_error) || Boolean(recovery)}
            >
              <Button
                danger
                loading={busy}
                disabled={alreadyRolledBack || Boolean(selectedRecord?.rollback_error) || Boolean(recovery)}
              >
                回滚这次写入
              </Button>
            </Popconfirm>
            <Button type="link" onClick={onDiscard}>
              关闭回执详情
            </Button>
          </Space>
          {rollbackReport && (
            <Alert
              type="warning"
              showIcon
              message="已回滚"
              description={`还原 ${rollbackReport.restored.length} 个、删除 ${rollbackReport.removed.length} 个`}
            />
          )}
        </>
      ) : plan ? (
        <>
          <Alert
            type="info"
            showIcon
            message="待放行计划已保存"
            description={`生成于 ${plan.created_at}；项目重载后仍可继续检查 diff。`}
          />
          <Space size={8} wrap>
            <Tag color="blue">计划指纹 {plan.plan_hash}</Tag>
            <Text type="secondary" style={{ fontSize: 12 }}>
              {plan.items.length} 个文件 · 写入 {willWrite.length} · 覆盖 {overwrites}
            </Text>
          </Space>
          <Collapse
            size="small"
            defaultActiveKey={plan.items.map((_, index) => String(index))}
            items={plan.items.map((item, index) => ({
              key: String(index),
              label: (
                <Space size={8}>
                  <Tag color={ACTION_TAG[item.action].color}>{ACTION_TAG[item.action].label}</Tag>
                  <Text>{item.rel_path}</Text>
                </Space>
              ),
              children: <DiffView item={item} />,
            }))}
          />
          <Space>
            <Popconfirm
              title="确认写入？"
              description={
                allSkip
                  ? "所有文件内容都已是最新，不会写入任何东西。"
                  : overwrites > 0
                    ? `将覆盖 ${overwrites} 个已有文件（写入前自动备份，可回滚）。`
                    : `将新建 ${willWrite.length} 个文件。`
              }
              okText="确认写入"
              cancelText="再想想"
              onConfirm={onApply}
              disabled={allSkip || Boolean(recovery)}
            >
              <Button type="primary" loading={busy} disabled={allSkip || Boolean(recovery)}>
                {allSkip ? "无需写入" : `放行写入（${willWrite.length} 个）`}
              </Button>
            </Popconfirm>
            <Button onClick={onDiscard} disabled={busy}>
              丢弃计划
            </Button>
          </Space>
          {overwrites > 0 && (
            <Alert
              type="warning"
              showIcon
              message={`其中 ${overwrites} 个是覆盖已有文件`}
              description="原内容按内容寻址存入 .dftwork/backup/；备份受保留上限约束，不是重要数据的唯一副本。"
            />
          )}
        </>
      ) : (
        <Empty description="还没有待放行计划。在左侧选中一个任务，点「以此为模板」生成；也可从下方找回历史回执。" />
      )}

      {receipts.length > 0 && (
        <div>
          <Text type="secondary" style={{ fontSize: 12 }}>
            最近 {receipts.length} 条回执（最多 100 条）
          </Text>
          <Select
            style={{ width: "100%", marginTop: 6 }}
            placeholder="选择历史回执查看或回滚"
            value={receipt ? receiptId(receipt) : undefined}
            onChange={(id) => {
              const record = receipts.find((candidate) => receiptId(candidate.receipt) === id);
              if (record) onSelectReceipt(record);
            }}
            options={receipts.map((record) => ({
              value: receiptId(record.receipt),
              label: `${record.receipt.finished_at} · ${record.receipt.plan_hash.slice(0, 10)} · ${
                record.rolled_back ? "已回滚" : record.rollback_error ? "备份不可用" : "可回滚"
              }`,
            }))}
          />
        </div>
      )}

      <Space size={8} wrap>
        <Text type="secondary" style={{ fontSize: 12 }}>
          备份占用 {(backupBytes / 1024 / 1024).toFixed(2)} MB · 每文件 5 版 / 500 MB / 30 天
        </Text>
        <Popconfirm
          title="按保留策略清理备份？"
          description="失去备份的回执会明确标为不可回滚；未完成写入的恢复批次不会被删除。"
          okText="清理"
          cancelText="取消"
          onConfirm={onPrune}
        >
          <Button size="small" loading={busy}>
            清理备份
          </Button>
        </Popconfirm>
      </Space>
    </Space>
  );
}

/** 计划条目的紧凑表格（供外部展示用，例如任务列表里的摘要）。 */
export function PlanSummaryTable({ plan }: { plan: BuildPlan }) {
  return (
    <Table
      size="small"
      pagination={false}
      rowKey="rel_path"
      dataSource={plan.items}
      columns={[
        {
          title: "动作",
          dataIndex: "action",
          width: 70,
          render: (a: BuildItem["action"]) => <Tag color={ACTION_TAG[a].color}>{ACTION_TAG[a].label}</Tag>,
        },
        { title: "文件", dataIndex: "rel_path" },
        { title: "说明", dataIndex: "reason" },
      ]}
    />
  );
}

/** 卡片外壳，供 App 里统一排版。 */
export function BuildCard(props: Parameters<typeof BuildPanel>[0]) {
  return (
    <Card size="small" title="构建与放行" styles={{ body: { padding: 10 } }}>
      <BuildPanel {...props} />
    </Card>
  );
}
