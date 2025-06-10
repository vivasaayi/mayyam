import React, { useState, useRef } from "react";
import { CCard, CCardBody, CCardHeader, CButton, CForm, CFormInput, CFormLabel, CSpinner } from "@coreui/react";
import { FormGroup } from "reactstrap";
import api from "../services/api";

const DEFAULT_MODEL = "gpt-3.5-turbo";

const ChatPage = () => {
  const [messages, setMessages] = useState([
    { role: "assistant", content: "Hello! How can I help you today?" },
  ]);
  const [input, setInput] = useState("");
  const [loading, setLoading] = useState(false);
  const [model, setModel] = useState(DEFAULT_MODEL);
  const [temperature, setTemperature] = useState(1.0);
  const messagesEndRef = useRef(null);

  const scrollToBottom = () => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  };

  const handleSend = async (e) => {
    e.preventDefault();
    if (!input.trim()) return;
    const newMessages = [...messages, { role: "user", content: input }];
    setMessages(newMessages);
    setInput("");
    setLoading(true);
    try {
      const res = await api.post("/api/ai/chat", {
        messages: newMessages,
        model,
        temperature,
      });
      const assistantMsg = res.data.choices?.[0]?.message || { role: "assistant", content: "[No response]" };
      setMessages([...newMessages, assistantMsg]);
      setTimeout(scrollToBottom, 100);
    } catch (err) {
      setMessages([...newMessages, { role: "assistant", content: "[Error: Could not get response]" }]);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="container py-4" style={{ maxWidth: 700 }}>
      <CCard>
        <CCardBody>
          <h4>AI Chat</h4>
          <div style={{ minHeight: 300, maxHeight: 400, overflowY: "auto", background: "#f8f9fa", padding: 16, borderRadius: 8 }}>
            {messages.map((msg, idx) => (
              <div key={idx} style={{ marginBottom: 12, textAlign: msg.role === "user" ? "right" : "left" }}>
                <span style={{ fontWeight: "bold", color: msg.role === "user" ? "#007bff" : "#28a745" }}>
                  {msg.role === "user" ? "You" : "Assistant"}:
                </span>
                <span style={{ marginLeft: 8, whiteSpace: "pre-wrap" }}>{msg.content}</span>
              </div>
            ))}
            <div ref={messagesEndRef} />
          </div>
          <CForm onSubmit={handleSend} className="mt-3">
            <FormGroup className="mb-2">
              <CFormLabel htmlFor="model">Model</CFormLabel>
              <CFormInput
                id="model"
                value={model}
                onChange={e => setModel(e.target.value)}
                placeholder="Model (e.g., gpt-3.5-turbo)"
              />
            </FormGroup>
            <FormGroup className="mb-2">
              <CFormLabel htmlFor="temperature">Temperature</CFormLabel>
              <CFormInput
                id="temperature"
                type="number"
                min={0}
                max={2}
                step={0.1}
                value={temperature}
                onChange={e => setTemperature(Number(e.target.value))}
              />
            </FormGroup>
            <FormGroup className="d-flex">
              <CFormInput
                value={input}
                onChange={e => setInput(e.target.value)}
                placeholder="Type your message..."
                disabled={loading}
                autoFocus
              />
              <CButton type="submit" color="primary" disabled={loading || !input.trim()} className="ms-2">
                {loading ? <CSpinner size="sm" /> : "Send"}
              </CButton>
            </FormGroup>
          </CForm>
        </CCardBody>
      </CCard>
    </div>
  );
};

export default ChatPage;
