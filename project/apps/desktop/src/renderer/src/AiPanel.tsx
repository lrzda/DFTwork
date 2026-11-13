import { useEffect, useRef, useState } from "react";
import { Alert, Button, Card, Empty, Input, Space, Spin, Typography } from "antd";
import type { AnalysisReport } from "@dftwork/shared";
import { api, describeError } from "./api";

const { Text, Paragraph } = Typography;
const LS_MODEL = "dftwork.ai.model";
const LS_PROVIDER = "dftwork.ai.provider";

type ChatMessage = {
  id: number;
  role: "user" | "assistant";
  text: string;
  report?: AnalysisReport;
  failed?: boolean;
};

export function AiPanel({
  projectRoot,
  hermesAvailable,
  hermesReason,
}: {
  projectRoot: string | null;
  hermesAvailable: boolean;
  hermesReason: string | null;
}) {
  const [model, setModel] = useState(() => localStorage.getItem(LS_MODEL) ?? "deepseek-v4-flash");
  const [provider, setProvider] = useState(() => localStorage.getItem(LS_PROVIDER) ?? "deepseek");
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [sessionId, setSessionId] = useState<string | undefined>();
  const [draft, setDraft] = useState("");
  const [busy, setBusy] = useState(false);
  const [elapsed, setElapsed] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const timer = useRef<number | null>(null);
  const request = useRef(0);
  const nextMessage = useRef(1);
  const messageList = useRef<HTMLDivElement>(null);

  const stopTimer = (): void => {
    if (timer.current !== null) {
      window.clearInterval(timer.current);
      timer.current = null;
    }
  };

  useEffect(() => {
    localStorage.setItem(LS_MODEL, model);
    localStorage.setItem(LS_PROVIDER, provider);
  }, [model, provider]);

  useEffect(() => {
    request.current += 1;
    stopTimer();
    setMessages([]);
    setSessionId(undefined);
    setBusy(false);
    setElapsed(0);
    setError(null);
  }, [projectRoot]);

  useEffect(
    () => () => {
      request.current += 1;
      stopTimer();
    },
    [],
  );

  useEffect(() => {
    const element = messageList.current;
    if (element) element.scrollTop = element.scrollHeight;
  }, [messages, busy]);

  const send = async (): Promise<void> => {
    const message = draft.trim();
    if (!projectRoot || !message || busy) return;
    const currentRequest = ++request.current;
    const started = Date.now();
    const nextId = nextMessage.current++;
    setMessages((current) => [...current, { id: nextId, role: "user", text: message }]);
    setDraft("");
    setBusy(true);
    setError(null);
    setElapsed(0);
    timer.current = window.setInterval(
      () => setElapsed(Math.floor((Date.now() - started) / 1000)),
      250,
    );
    try {
      const report = await api.hermesChat(
        projectRoot,
        message,
        sessionId,
        model.trim() || undefined,
        provider.trim() || undefined,
        2,
      );
      if (currentRequest !== request.current) return;
      const failed = report.timed_out || report.exit_code !== 0;
      if (!failed && report.session_id) setSessionId(report.session_id);
      setMessages((current) => [
        ...current,
        {
          id: nextMessage.current++,
          role: "assistant",
          text: report.text || (failed ? "Hermes 没有返回更多错误信息" : "Hermes 没有返回文本"),
          report,
          failed,
        },
      ]);
    } catch (e) {
      if (currentRequest === request.current) setError(describeError(e));
    } finally {
      if (currentRequest === request.current) {
        stopTimer();
        setBusy(false);
      }
    }
  };

  if (!hermesAvailable) {
    return (
      <Empty
        description={
          <Space direction="vertical" size={4}>
            <Text>本机没有检测到 Hermes，对话不可用</Text>
            <Text type="secondary" style={{ fontSize: 12 }}>
              {hermesReason ?? "安装 Hermes 后重启本软件即可；其余功能不受影响"}
            </Text>
          </Space>
        }
      />
    );
  }

  if (!projectRoot) {
    return <Empty description="先扫描一个项目，再开始 Hermes 对话" />;
  }

  return (
    <div style={{ height: "100%", minHeight: 0, display: "flex", flexDirection: "column", gap: 10 }}>
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
        <Button
          size="small"
          disabled={busy || messages.length === 0}
          onClick={() => {
            request.current += 1;
            setMessages([]);
            setSessionId(undefined);
            setError(null);
          }}
        >
          新对话
        </Button>
      </Space>

      <Alert
        type="info"
        showIcon
        message="Hermes 在当前扫描项目根目录中工作"
        description={
          <span style={{ fontSize: 12, wordBreak: "break-all" }}>
            {projectRoot}。当前只开放文本对话，不提供文件写入或终端工具。
          </span>
        }
      />

      {error && (
        <Alert
          type="error"
          showIcon
          closable
          message="消息发送失败"
          description={error}
          onClose={() => setError(null)}
        />
      )}

      <div
        ref={messageList}
        style={{ flex: 1, minHeight: 160, overflowY: "auto", padding: "4px 2px" }}
      >
        {messages.length === 0 && !busy ? (
          <Empty description="输入消息，直接和 Hermes 对话" />
        ) : (
          <Space direction="vertical" size={8} style={{ width: "100%" }}>
            {messages.map((message) => (
              <div
                key={message.id}
                style={{ display: "flex", justifyContent: message.role === "user" ? "flex-end" : "flex-start" }}
              >
                <Card
                  size="small"
                  style={{ maxWidth: "92%", width: message.role === "user" ? "fit-content" : "100%" }}
                  styles={{ body: { padding: "8px 12px" } }}
                >
                  <Text type="secondary" style={{ fontSize: 11 }}>
                    {message.role === "user" ? "你" : "Hermes"}
                  </Text>
                  <Paragraph style={{ whiteSpace: "pre-wrap", margin: "4px 0 0" }}>
                    {message.text}
                  </Paragraph>
                  {message.failed && (
                    <Alert
                      type="warning"
                      showIcon
                      style={{ marginTop: 8 }}
                      message={message.report?.timed_out ? "本次对话超时，回答可能不完整" : `Hermes 返回错误（exit_code=${message.report?.exit_code ?? -1}）`}
                    />
                  )}
                  {message.report && (
                    <Text type="secondary" style={{ display: "block", marginTop: 6, fontSize: 11 }}>
                      {message.report.tokens.total.toLocaleString()} tokens · {(message.report.duration_ms / 1000).toFixed(1)} s
                    </Text>
                  )}
                </Card>
              </div>
            ))}
          </Space>
        )}
        {busy && (
          <Space style={{ marginTop: 10 }}>
            <Spin size="small" />
            <Text type="secondary" style={{ fontSize: 12 }}>正在等待 Hermes 回答，已等 {elapsed} 秒</Text>
          </Space>
        )}
      </div>

      <Space.Compact style={{ width: "100%", alignItems: "flex-start" }}>
        <Input.TextArea
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onPressEnter={(e) => {
            if (!e.shiftKey) {
              e.preventDefault();
              void send();
            }
          }}
          autoSize={{ minRows: 2, maxRows: 5 }}
          placeholder="给 Hermes 发消息；Enter 发送，Shift+Enter 换行"
          disabled={busy}
        />
        <Button
          type="primary"
          loading={busy}
          disabled={!draft.trim() || !model.trim() || !provider.trim()}
          onClick={() => void send()}
          style={{ height: "auto", alignSelf: "stretch" }}
        >
          发送
        </Button>
      </Space.Compact>
      <Text type="secondary" style={{ fontSize: 11 }}>
        {sessionId ? "后续消息会续接当前 Hermes 会话。" : "首次回答后将续接同一会话。"}切换项目会开始新会话；聊天记录只保留在当前页面内存中。
      </Text>
    </div>
  );
}
