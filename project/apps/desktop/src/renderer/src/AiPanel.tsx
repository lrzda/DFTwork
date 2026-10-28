/**
 * AI 分析面板。
 *
 * 三条设计约束，都来自实测：
 * 1. **AI 不持有任何工具**——数据由我们收集注入，它只能做纯文本分析。
 *    它想改东西只能给文字建议，用户照着走「以此为模板」→ M4 放行。
 * 2. **成本可见**：一次冷启动约 18k input tokens，界面必须显示实际用量。
 * 3. **失败要说清是哪种失败**：模型不存在时进程正常退出但 result.exit_code≠0，
 *    超时是被我们 kill 的——两者提示不同。
 */
import { useEffect, useRef, useState } from "react";
import { Alert, Button, Card, Descriptions, Empty, Input, Space, Spin, Tag, Typography } from "antd";
import type { AnalysisReport } from "@dftwork/shared";
import { api, describeError } from "./api";

const { Text, Paragraph } = Typography;

const LS_MODEL = "dftwork.ai.model";
const LS_PROVIDER = "dftwork.ai.provider";

export function AiPanel({
  taskAbsPath,
  taskName,
  hermesAvailable,
  hermesReason,
}: {
  taskAbsPath: string | null;
  taskName: string | null;
  hermesAvailable: boolean;
  hermesReason: string | null;
}) {
  const [model, setModel] = useState(() => localStorage.getItem(LS_MODEL) ?? "deepseek-v4-flash");
  const [provider, setProvider] = useState(() => localStorage.getItem(LS_PROVIDER) ?? "deepseek");
  const [busy, setBusy] = useState(false);
  const [elapsed, setElapsed] = useState(0);
  const [report, setReport] = useState<AnalysisReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const timer = useRef<number | null>(null);

  useEffect(() => {
    localStorage.setItem(LS_MODEL, model);
    localStorage.setItem(LS_PROVIDER, provider);
  }, [model, provider]);

  // 换任务时清掉上一次结果，否则会把上一个任务的结论当成这个任务的
  useEffect(() => {
    setReport(null);
    setError(null);
    setElapsed(0);
  }, [taskAbsPath]);

  useEffect(
    () => () => {
      if (timer.current !== null) window.clearInterval(timer.current);
    },
    [],
  );

  const run = async (): Promise<void> => {
    if (!taskAbsPath) return;
    setBusy(true);
    setError(null);
    setReport(null);
    setElapsed(0);
    const started = Date.now();
    timer.current = window.setInterval(() => setElapsed(Math.floor((Date.now() - started) / 1000)), 250);
    try {
      const r = await api.analyze(taskAbsPath, model.trim() || undefined, provider.trim() || undefined, 2);
      setReport(r);
    } catch (e) {
      setError(describeError(e));
    } finally {
      if (timer.current !== null) {
        window.clearInterval(timer.current);
        timer.current = null;
      }
      setBusy(false);
    }
  };

  if (!hermesAvailable) {
    return (
      <Empty
        description={
          <Space direction="vertical" size={4}>
            <Text>本机没有检测到 Hermes，AI 分析不可用</Text>
            <Text type="secondary" style={{ fontSize: 12 }}>
              {hermesReason ?? "安装 Hermes 后重启本软件即可；其余功能不受影响"}
            </Text>
          </Space>
        }
      />
    );
  }

  if (!taskAbsPath) {
    return <Empty description="在左侧选一个任务" />;
  }

  return (
    <Space direction="vertical" size={10} style={{ width: "100%" }}>
      <Space size={6} wrap>
        <Input
          size="small"
          addonBefore="模型"
          value={model}
          onChange={(e) => setModel(e.target.value)}
          style={{ width: 230 }}
          disabled={busy}
        />
        <Input
          size="small"
          addonBefore="提供方"
          value={provider}
          onChange={(e) => setProvider(e.target.value)}
          style={{ width: 180 }}
          disabled={busy}
        />
        <Button type="primary" size="small" loading={busy} onClick={() => void run()}>
          {busy ? `分析中 ${elapsed}s` : "分析此任务"}
        </Button>
      </Space>

      <Alert
        type="info"
        showIcon
        message="AI 只做分析，不会改任何文件"
        description={
          <span style={{ fontSize: 12 }}>
            它没有文件、终端或联网工具，看不到的只有我们喂进去的任务数据。
            若它建议修改参数，请用左侧「以此为模板」生成计划，看过 diff 再放行。
          </span>
        }
      />

      {error && <Alert type="error" showIcon message="分析失败" description={error} />}

      {busy && !report && (
        <Space>
          <Spin size="small" />
          <Text type="secondary" style={{ fontSize: 12 }}>
            已等 {elapsed} 秒（一次冷启动约 18k tokens，通常 10–40 秒）
          </Text>
        </Space>
      )}

      {report && (
        <>
          {report.timed_out ? (
            <Alert type="warning" showIcon message="超时，已中止" description="子进程被强制结束，下面的内容可能不完整。" />
          ) : report.exit_code !== 0 ? (
            <Alert
              type="error"
              showIcon
              message={`模型返回错误（exit_code=${report.exit_code}）`}
              description={report.text || "没有更多信息"}
            />
          ) : null}

          {report.exit_code === 0 && !report.timed_out && (
            <Card size="small" title={taskName ? `分析结果 · ${taskName}` : "分析结果"} styles={{ body: { padding: 12 } }}>
              <Paragraph style={{ whiteSpace: "pre-wrap", marginBottom: 0 }}>{report.text}</Paragraph>
            </Card>
          )}

          <Descriptions size="small" column={2} bordered>
            <Descriptions.Item label="输入 tokens">{report.tokens.input.toLocaleString()}</Descriptions.Item>
            <Descriptions.Item label="输出 tokens">{report.tokens.output.toLocaleString()}</Descriptions.Item>
            <Descriptions.Item label="Hermes 耗时">
              {(report.duration_ms / 1000).toFixed(1)} s
            </Descriptions.Item>
            <Descriptions.Item label="事件行数">{report.event_lines}</Descriptions.Item>
            <Descriptions.Item label="会话" span={2}>
              <Text code style={{ fontSize: 11 }}>
                {report.session_id || "—"}
              </Text>
            </Descriptions.Item>
          </Descriptions>

          {report.tool_calls.length > 0 && (
            <Card size="small" title="AI 调用过的工具" styles={{ body: { padding: 8 } }}>
              <Space direction="vertical" size={2}>
                {report.tool_calls.map((t, i) => (
                  <Text key={i} style={{ fontSize: 12 }}>
                    <Tag color={t.is_error ? "red" : "green"}>{t.name}</Tag>
                    <Text type="secondary">{t.summary}</Text>
                  </Text>
                ))}
              </Space>
            </Card>
          )}
        </>
      )}

      <Paragraph type="secondary" style={{ fontSize: 11, marginBottom: 0 }}>
        每轮独立会话。同一会话续接会让输入 tokens 翻倍（实测 17.8k → 35.7k），
        所以默认不续接；需要追问请重新分析并在提示词里补充上下文。
      </Paragraph>
    </Space>
  );
}
