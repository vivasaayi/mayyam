// Copyright (c) 2025 Rajan Panneer Selvam
//
// Licensed under the Business Source License 1.1 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     https://www.mariadb.com/bsl11
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.


import React, { useState, useRef, useEffect } from "react";
import { CCard, CCardBody, CCardHeader, CButton, CForm, CFormInput, CFormLabel, CSpinner, CFormSelect, CAlert } from "@coreui/react";
import { FormGroup } from "reactstrap";
import api, { fetchWithAuth } from "../services/api";

const DEFAULT_MODEL = "gemma-3-27b-it";

const ChatPage = () => {
  const [messages, setMessages] = useState([
    { role: "assistant", content: "Hello! How can I help you today?" },
  ]);
  const [input, setInput] = useState("");
  const [loading, setLoading] = useState(false);
  const [model, setModel] = useState(DEFAULT_MODEL);
  const [temperature, setTemperature] = useState(1.0);
  const [availableModels, setAvailableModels] = useState([]);
  const [modelsLoading, setModelsLoading] = useState(true);
  const [error, setError] = useState(null);
  const messagesEndRef = useRef(null);
  const controllerRef = useRef(null);
  const MAX_MESSAGE_LEN = 4000;
  const [isStreaming, setIsStreaming] = useState(false);

  // Fetch available models on component mount
  useEffect(() => {
    const fetchModels = async () => {
      try {
        const response = await api.get("/api/v1/llm-providers?active_only=true");
        const models = response.data.providers.map(provider => ({
          value: provider.model_name,
          label: `${provider.name} (${provider.model_name})`,
          provider_type: provider.provider_type
        }));
        setAvailableModels(models);
        
        // Set default model to the first available model if current default is not available
        if (models.length > 0 && !models.find(m => m.value === DEFAULT_MODEL)) {
          setModel(models[0].value);
        }
      } catch (error) {
        console.error("Failed to fetch models:", error);
        // Fallback models if API fails
        setAvailableModels([
          { value: "gemma-3-27b-it", label: "Local Host (gemma-3-27b-it)", provider_type: "local" },
          { value: "gpt-3.5-turbo", label: "OpenAI GPT-3.5 Turbo", provider_type: "openai" },
        ]);
      }
      setModelsLoading(false);
    };

    fetchModels();
  }, []);

  const scrollToBottom = () => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  };

  const handleSend = async (e) => {
    e.preventDefault();
    setError(null);
    if (!input.trim()) return;
    if (input.length > MAX_MESSAGE_LEN) {
      setError(`Message too long (max ${MAX_MESSAGE_LEN} characters).`);
      return;
    }
    // Prepare conversation with user message and an empty assistant placeholder for streaming
    const baseMessages = [...messages, { role: "user", content: input }];
    setMessages([...baseMessages, { role: "assistant", content: "" }]);
    setInput("");
    setLoading(true);
    setIsStreaming(true);
    const controller = new AbortController();
    controllerRef.current = controller;
    try {
      const response = await fetchWithAuth(`/api/ai/chat/stream`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ messages: baseMessages, model, temperature }),
        signal: controller.signal,
      });

      if (!response.ok || !response.body) {
        // Fallback to non-streaming call
        const res = await api.post("/api/ai/chat", {
          messages: baseMessages,
          model,
          temperature,
        });
        const assistantMsg = res.data.choices?.[0]?.message || { role: "assistant", content: "[No response]" };
        setMessages([...baseMessages, assistantMsg]);
        return;
      }

      const reader = response.body.getReader();
      const decoder = new TextDecoder("utf-8");
      let buffer = "";

      while (true) {
        const { value, done } = await reader.read();
        if (done) break;
        buffer += decoder.decode(value, { stream: true });
        const parts = buffer.split("\n\n");
        buffer = parts.pop() || ""; // keep partial chunk
        for (const part of parts) {
          // Expect lines like: "data: <text>" or event blocks
          const lines = part.split("\n");
          const dataLine = lines.find((l) => l.startsWith("data: "));
          if (!dataLine) continue;
          const data = dataLine.slice(6);
          if (!data) continue;
          // Append streamed text to the last assistant message
          setMessages((prev) => {
            const updated = [...prev];
            const lastIdx = updated.length - 1;
            if (lastIdx >= 0 && updated[lastIdx].role === "assistant") {
              updated[lastIdx] = {
                ...updated[lastIdx],
                content: (updated[lastIdx].content || "") + data,
              };
            }
            return updated;
          });
        }
        // Keep view scrolled near the bottom while streaming
        scrollToBottom();
      }
    } catch (err) {
      if (err?.name === "AbortError") {
        // User cancelled streaming; keep partial content
      } else {
        console.error("Streaming error:", err);
        setError(err?.message || "Streaming failed");
      }
    } finally {
      setLoading(false);
      setIsStreaming(false);
      controllerRef.current = null;
      setTimeout(scrollToBottom, 100);
    }
  };

  const handleStop = () => {
    try {
      controllerRef.current?.abort();
    } catch (_) {
      // ignore
    }
  };

  const handleCopy = async () => {
    try {
      const text = messages.map(m => `${m.role}: ${m.content}`).join("\n\n");
      await navigator.clipboard.writeText(text);
      setError(null);
    } catch (e) {
      setError("Failed to copy conversation to clipboard.");
    }
  };

  const handleClear = () => {
    setMessages([{ role: "assistant", content: "Hello! How can I help you today?" }]);
    setError(null);
  };

  return (
    <div style={{ 
      height: "calc(100vh - 120px)", // Account for header height
      display: "flex", 
      flexDirection: "column",
      margin: "-1rem", // Counteract container padding
      marginTop: "-1.5rem" // Account for header margin
    }}>
      <CCard style={{ 
        flex: 1, 
        border: "none", 
        borderRadius: 0,
        height: "100%"
      }}>
        <CCardHeader style={{ 
          backgroundColor: "#f8f9fa", 
          borderBottom: "1px solid #dee2e6",
          padding: "1rem 1.5rem"
        }}>
          <h4 className="mb-0">AI Chat</h4>
          <div className="mt-2 d-flex gap-2">
            <CButton color="secondary" size="sm" variant="outline" onClick={handleCopy} disabled={messages.length === 0}>Copy</CButton>
            {isStreaming ? (
              <CButton color="warning" size="sm" variant="outline" onClick={handleStop} disabled={!isStreaming}>Stop</CButton>
            ) : null}
            <CButton color="danger" size="sm" variant="outline" onClick={handleClear} disabled={loading || isStreaming}>Clear</CButton>
          </div>
        </CCardHeader>
        <CCardBody style={{ 
          flex: 1, 
          display: "flex", 
          flexDirection: "column", 
          padding: "1.5rem",
          height: "calc(100% - 70px)" // Account for header
        }}>
          {/* Error Banner */}
          {error && (
            <CAlert color="danger" className="mb-3">{error}</CAlert>
          )}

          {/* Chat Messages Area */}
          <div style={{ 
            flex: 1, 
            overflowY: "auto", 
            background: "#ffffff", 
            border: "1px solid #dee2e6",
            padding: "1.5rem", 
            borderRadius: "12px", 
            marginBottom: "1.5rem",
            minHeight: "400px",
            boxShadow: "inset 0 1px 3px rgba(0,0,0,0.05)"
          }}>
            {messages.map((msg, idx) => (
              <div key={idx} style={{ 
                marginBottom: 20, 
                display: "flex", 
                justifyContent: msg.role === "user" ? "flex-end" : "flex-start" 
              }}>
                <div style={{
                  maxWidth: "75%",
                  padding: "14px 18px",
                  borderRadius: msg.role === "user" ? "20px 20px 5px 20px" : "20px 20px 20px 5px",
                  backgroundColor: msg.role === "user" ? "#007bff" : "#28a745",
                  color: "white",
                  wordWrap: "break-word",
                  whiteSpace: "pre-wrap",
                  boxShadow: "0 2px 8px rgba(0,0,0,0.1)",
                  fontSize: "1rem",
                  lineHeight: "1.4"
                }}>
                  <div style={{ 
                    fontSize: "0.75rem", 
                    opacity: 0.9, 
                    marginBottom: "6px",
                    fontWeight: "500"
                  }}>
                    {msg.role === "user" ? "You" : "Assistant"}
                  </div>
                  {msg.content}
                </div>
              </div>
            ))}
            <div ref={messagesEndRef} />
          </div>

          {/* Controls and Input Area */}
          <div style={{ flexShrink: 0, backgroundColor: "#f8f9fa", padding: "1rem", borderRadius: "12px" }}>
            {/* Model and Temperature Controls */}
            <div style={{ display: "flex", gap: "1.5rem", marginBottom: "1rem" }}>
              <FormGroup style={{ flex: 1 }}>
                <CFormLabel htmlFor="model" style={{ fontWeight: "600", marginBottom: "0.5rem" }}>
                  Model
                </CFormLabel>
                {modelsLoading ? (
                  <CFormSelect disabled style={{ fontSize: "1rem" }}>
                    <option>Loading models...</option>
                  </CFormSelect>
                ) : (
                  <CFormSelect
                    id="model"
                    value={model}
                    onChange={e => setModel(e.target.value)}
                    style={{ fontSize: "1rem" }}
                  >
                    {availableModels.map(modelOption => (
                      <option key={modelOption.value} value={modelOption.value}>
                        {modelOption.label}
                      </option>
                    ))}
                  </CFormSelect>
                )}
              </FormGroup>
              <FormGroup style={{ width: "180px" }}>
                <CFormLabel htmlFor="temperature" style={{ fontWeight: "600", marginBottom: "0.5rem" }}>
                  Temperature
                </CFormLabel>
                <CFormInput
                  id="temperature"
                  type="number"
                  min={0}
                  max={2}
                  step={0.1}
                  value={temperature}
                  onChange={e => setTemperature(Number(e.target.value))}
                  style={{ fontSize: "1rem" }}
                />
              </FormGroup>
            </div>

            {/* Message Input */}
            <CForm onSubmit={handleSend}>
              <FormGroup className="d-flex" style={{ gap: "0.75rem" }}>
                <CFormInput
                  value={input}
                  onChange={e => setInput(e.target.value)}
                  placeholder="Type your message..."
                  disabled={loading}
                  autoFocus
                  style={{ 
                    fontSize: "1rem", 
                    padding: "0.75rem 1rem",
                    borderRadius: "25px",
                    border: "2px solid #e9ecef"
                  }}
                />
                <CButton 
                  type="submit" 
                  color="primary" 
                  disabled={loading || !input.trim()} 
                  style={{ 
                    minWidth: "100px",
                    borderRadius: "25px",
                    padding: "0.75rem 1.5rem",
                    fontWeight: "600"
                  }}
                >
                  {loading ? <CSpinner size="sm" /> : "Send"}
                </CButton>
              </FormGroup>
            </CForm>
          </div>
        </CCardBody>
      </CCard>
    </div>
  );
};

export default ChatPage;
