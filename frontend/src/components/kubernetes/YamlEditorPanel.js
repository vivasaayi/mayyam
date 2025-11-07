import React, { useEffect, useMemo, useState } from "react";
import {
  CAlert,
  CButton,
  CCard,
  CCardBody,
  CCardHeader,
  CFormTextarea,
  CSpinner,
} from "@coreui/react";
import yaml from "js-yaml";

const YamlEditorPanel = ({
  title,
  resourceType,
  resourceSummary,
  resource,
  namespace,
  isNew,
  loading,
  error,
  onSave,
  onDelete,
  onCancel,
}) => {
  const [yamlContent, setYamlContent] = useState("");
  const [validationError, setValidationError] = useState(null);

  const docTitle = useMemo(() => {
    const suffix = resourceSummary?.name ? `: ${resourceSummary.name}` : "";
    return `${title}${suffix}`;
  }, [resourceSummary?.name, title]);

  useEffect(() => {
    if (loading) {
      return;
    }
    const baseDoc = buildBaseDocument({
      resourceType,
      resourceSummary,
      resource,
      namespace,
      isNew,
    });
    try {
      const dumped = yaml.dump(baseDoc, { noRefs: true, lineWidth: 120 });
      setYamlContent(dumped);
      setValidationError(null);
    } catch (dumpError) {
      console.error("Failed to serialize YAML", dumpError);
      setYamlContent("# Unable to render resource as YAML\n{}");
      setValidationError("Unable to render current resource as YAML");
    }
  }, [resourceType, resourceSummary, resource, namespace, isNew, loading]);

  const handleSave = () => {
    try {
      const parsed = yaml.load(yamlContent) || {};
      validateDocument(resourceType, parsed);
      setValidationError(null);
      onSave?.(parsed);
    } catch (err) {
      console.error("YAML validation failed", err);
      setValidationError(err.message || "Unable to parse YAML document");
    }
  };

  const showDelete = !isNew && Boolean(onDelete);

  return (
    <CCard className="h-100">
      <CCardHeader className="d-flex justify-content-between align-items-center">
        <strong>{docTitle}</strong>
        <div className="d-flex gap-2">
          {showDelete && (
            <CButton color="danger" variant="outline" size="sm" disabled={loading} onClick={onDelete}>
              Delete
            </CButton>
          )}
          <CButton color="secondary" variant="outline" size="sm" onClick={onCancel} disabled={loading}>
            Cancel
          </CButton>
          <CButton color="primary" size="sm" onClick={handleSave} disabled={loading}>
            {loading ? <CSpinner size="sm" /> : "Save"}
          </CButton>
        </div>
      </CCardHeader>
      <CCardBody className="d-flex flex-column">
        {error && <CAlert color="danger">{error}</CAlert>}
        {validationError && <CAlert color="warning">{validationError}</CAlert>}
        {resourceType === "secret" && (
          <CAlert color="info" className="mb-3">
            Secret values are saved as plaintext in this editor and encoded server-side. Existing values may appear as "***" when fetched.
          </CAlert>
        )}
        <CFormTextarea
          rows={18}
          value={yamlContent}
          onChange={(e) => setYamlContent(e.target.value)}
          disabled={loading}
          style={{ fontFamily: "monospace", fontSize: "0.85rem" }}
        />
      </CCardBody>
    </CCard>
  );
};

const buildBaseDocument = ({ resourceType, resourceSummary, resource, namespace, isNew }) => {
  const fallbackName = resourceSummary?.name || "";
  const resolvedNamespace =
    resource?.metadata?.namespace ||
    resourceSummary?.namespace ||
    (namespace && namespace !== "all" ? namespace : "");

  const metadata = {
    name: resource?.metadata?.name || fallbackName,
    namespace: resolvedNamespace,
    labels: resource?.metadata?.labels || resourceSummary?.labels || undefined,
    annotations: resource?.metadata?.annotations || resourceSummary?.annotations || undefined,
  };

  if (resourceType === "secret") {
    return {
      apiVersion: resource?.apiVersion || "v1",
      kind: resource?.kind || "Secret",
      metadata,
      type: resource?.type || resource?.type_field || resourceSummary?.type_field || undefined,
      data: mapSecretData(resource?.data, isNew),
    };
  }

  return {
    apiVersion: resource?.apiVersion || "v1",
    kind: resource?.kind || "ConfigMap",
    metadata,
    data: resource?.data || {},
  };
};

const mapSecretData = (data, isNew) => {
  if (!data || Object.keys(data).length === 0) {
    return isNew ? { example: "change-me" } : {};
  }
  const entries = Object.entries(data).map(([key, value]) => {
    const stringValue = normaliseSecretValue(value);
    return [key, stringValue];
  });
  return Object.fromEntries(entries);
};

const normaliseSecretValue = (value) => {
  if (typeof value === "string") {
    return decodeBase64(value) ?? value;
  }
  if (value && typeof value === "object" && "0" in value) {
    return decodeBase64(value[0]) ?? "***";
  }
  if (value instanceof Uint8Array) {
    if (typeof TextDecoder === "undefined") {
      return "***";
    }
    try {
      return new TextDecoder().decode(value);
    } catch (err) {
      return "***";
    }
  }
  return "***";
};

const decodeBase64 = (input) => {
  if (!input || typeof input !== "string") {
    return null;
  }
  try {
    if (typeof atob === "function") {
      return atob(input);
    }
    if (typeof Buffer !== "undefined") {
      return Buffer.from(input, "base64").toString("utf8");
    }
  } catch (err) {
    return null;
  }
  return null;
};

const validateDocument = (resourceType, doc) => {
  if (!doc || typeof doc !== "object") {
    throw new Error("YAML must define an object");
  }
  const metadata = doc.metadata || {};
  if (!metadata.name || typeof metadata.name !== "string") {
    throw new Error("metadata.name is required");
  }
  if (!metadata.namespace || typeof metadata.namespace !== "string") {
    throw new Error("metadata.namespace is required");
  }
  if (resourceType === "configmap") {
    if (doc.data && typeof doc.data !== "object") {
      throw new Error("ConfigMap data must be an object of key/value pairs");
    }
  } else if (resourceType === "secret") {
    if (doc.data && typeof doc.data !== "object") {
      throw new Error("Secret data must be an object of key/value pairs");
    }
  }
};

export default YamlEditorPanel;
