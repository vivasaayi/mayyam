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


import React, { useCallback, useEffect, useMemo, useState } from "react";
import {
  CAlert,
  CBadge,
  CButton,
  CCard,
  CCardBody,
  CCardHeader,
  CCol,
  CListGroup,
  CListGroupItem,
  CNav,
  CNavItem,
  CNavLink,
  CRow,
  CSpinner,
} from "@coreui/react";
import {
  deleteConfigMap,
  deleteSecret,
  getConfigMap,
  getConfigMaps,
  getSecret,
  getSecrets,
  upsertConfigMap,
  upsertSecret,
} from "../../services/kubernetesApiService";
import YamlEditorPanel from "./YamlEditorPanel";

const ConfigMapsSecretsManager = ({ clusterId, namespace }) => {
  const [activeType, setActiveType] = useState("configmap");
  const [items, setItems] = useState([]);
  const [loadingList, setLoadingList] = useState(false);
  const [listError, setListError] = useState(null);
  const [selectedSummary, setSelectedSummary] = useState(null);
  const [resource, setResource] = useState(null);
  const [editorLoading, setEditorLoading] = useState(false);
  const [editorError, setEditorError] = useState(null);
  const [isNew, setIsNew] = useState(false);
  const [statusMessage, setStatusMessage] = useState(null);
  const [statusVariant, setStatusVariant] = useState("success");

  const effectiveNamespace = useMemo(() => {
    if (!namespace || namespace === "") {
      return "all";
    }
    return namespace;
  }, [namespace]);

  useEffect(() => {
    setItems([]);
    setSelectedSummary(null);
    setResource(null);
    setStatusMessage(null);
  }, [clusterId, effectiveNamespace, activeType]);

  const fetchItems = useCallback(async () => {
    if (!clusterId) {
      return;
    }
    setLoadingList(true);
    setListError(null);
    try {
      if (activeType === "configmap") {
        const list = await getConfigMaps(clusterId, effectiveNamespace);
        setItems(list || []);
      } else {
        const list = await getSecrets(clusterId, effectiveNamespace);
        setItems(list || []);
      }
    } catch (err) {
      console.error("Failed to load config resources", err);
      setListError(err.message || "Unable to load resources");
      setItems([]);
    } finally {
      setLoadingList(false);
    }
  }, [activeType, clusterId, effectiveNamespace]);

  const loadResource = useCallback(
    async (summary, typeOverride) => {
      if (!clusterId || !summary) {
        return;
      }
      const type = typeOverride || activeType;
      const namespaceForItem = summary.namespace && summary.namespace !== ""
        ? summary.namespace
        : effectiveNamespace;
      if (!namespaceForItem || namespaceForItem === "all") {
        setEditorError("Namespace is required to load this resource.");
        setResource(null);
        return;
      }
      setEditorLoading(true);
      setEditorError(null);
      try {
        if (type === "configmap") {
          const data = await getConfigMap(clusterId, namespaceForItem, summary.name);
          setResource(data);
          setSelectedSummary({ ...summary, namespace: namespaceForItem });
        } else {
          const data = await getSecret(clusterId, namespaceForItem, summary.name);
          setResource(data);
          setSelectedSummary({ ...summary, namespace: namespaceForItem });
        }
      } catch (err) {
        console.error("Failed to load resource", err);
        setEditorError(err.message || "Unable to load resource");
        setResource(null);
      } finally {
        setEditorLoading(false);
      }
    },
    [activeType, clusterId, effectiveNamespace]
  );

  const refreshAfterMutation = useCallback(
    async (summaryOverride) => {
      await fetchItems();
      const summaryToRefresh = summaryOverride || selectedSummary;
      if (summaryToRefresh) {
        await loadResource(summaryToRefresh, activeType);
      }
    },
    [activeType, fetchItems, loadResource, selectedSummary]
  );

  useEffect(() => {
    fetchItems();
  }, [fetchItems]);

  const handleSelect = async (summary) => {
    setSelectedSummary(summary);
    setIsNew(false);
    setStatusMessage(null);
    await loadResource(summary);
  };

  const handleCreate = () => {
    if (!clusterId) {
      return;
    }
    const baseNamespace =
      effectiveNamespace && effectiveNamespace !== "all" ? effectiveNamespace : "";
    if (!baseNamespace) {
      setStatusVariant("warning");
      setStatusMessage("Select a namespace to create new resources.");
      return;
    }
    const summary = { name: "", namespace: baseNamespace };
    setSelectedSummary(summary);
    if (activeType === "configmap") {
      setResource({
        apiVersion: "v1",
        kind: "ConfigMap",
        metadata: { name: "", namespace: baseNamespace },
        data: {},
      });
    } else {
      setResource({
        apiVersion: "v1",
        kind: "Secret",
        metadata: { name: "", namespace: baseNamespace },
        type: "Opaque",
        data: {},
      });
    }
    setIsNew(true);
    setEditorError(null);
    setStatusMessage(null);
  };

  const handleCancel = () => {
    setSelectedSummary(null);
    setResource(null);
    setIsNew(false);
    setEditorError(null);
    setStatusMessage(null);
  };

  const handleDelete = async () => {
    if (!clusterId || !selectedSummary) {
      return;
    }
    const namespaceForItem = selectedSummary.namespace;
    if (!namespaceForItem || namespaceForItem === "all") {
      setEditorError("Namespace information is missing for deletion.");
      return;
    }
    setEditorLoading(true);
    setEditorError(null);
    try {
      if (activeType === "configmap") {
        await deleteConfigMap(clusterId, namespaceForItem, selectedSummary.name);
      } else {
        await deleteSecret(clusterId, namespaceForItem, selectedSummary.name);
      }
      setStatusVariant("success");
      setStatusMessage(`${capitalize(activeType)} deleted successfully.`);
      handleCancel();
      await fetchItems();
    } catch (err) {
      console.error("Failed to delete resource", err);
      setEditorError(err.message || "Unable to delete resource");
    } finally {
      setEditorLoading(false);
    }
  };

  const handleSave = async (doc) => {
    if (!clusterId) {
      return;
    }
    const name = doc.metadata?.name?.trim();
    const ns = doc.metadata?.namespace?.trim();
    if (!name || !ns) {
      setEditorError("metadata.name and metadata.namespace are required");
      return;
    }
    setEditorLoading(true);
    setEditorError(null);
    try {
      if (activeType === "configmap") {
        await upsertConfigMap(clusterId, ns, name, {
          data: ensureStringMap(doc.data || {}),
          labels: doc.metadata?.labels || null,
          annotations: doc.metadata?.annotations || null,
        });
      } else {
        await upsertSecret(clusterId, ns, name, {
          type_field: doc.type || doc.type_field || null,
          data: ensureStringMap(doc.data || {}),
          labels: doc.metadata?.labels || null,
          annotations: doc.metadata?.annotations || null,
        });
      }
      const updatedSummary = {
        name,
        namespace: ns,
        type_field:
          activeType === "secret"
            ? doc.type || doc.type_field || selectedSummary?.type_field
            : undefined,
        data_keys:
          activeType === "configmap"
            ? Object.keys(doc.data || {})
            : selectedSummary?.data_keys,
      };
      setStatusVariant("success");
      setStatusMessage(`${capitalize(activeType)} saved.`);
      setSelectedSummary((prev) => ({
        ...(prev || {}),
        ...updatedSummary,
      }));
      setIsNew(false);
      await refreshAfterMutation(updatedSummary);
    } catch (err) {
      console.error("Failed to save resource", err);
      setEditorError(err.message || "Unable to save resource");
    } finally {
      setEditorLoading(false);
    }
  };

  if (!clusterId) {
    return <CAlert color="info">Select a cluster to manage configuration resources.</CAlert>;
  }

  return (
    <CCard>
      <CCardHeader className="d-flex justify-content-between align-items-center">
        <strong>ConfigMaps & Secrets</strong>
        <div className="d-flex gap-2">
          <CButton color="primary" size="sm" variant="outline" onClick={handleCreate}>
            Create {activeType === "configmap" ? "ConfigMap" : "Secret"}
          </CButton>
          <CButton color="secondary" size="sm" variant="outline" onClick={fetchItems} disabled={loadingList}>
            {loadingList ? <CSpinner size="sm" /> : "Refresh"}
          </CButton>
        </div>
      </CCardHeader>
      <CCardBody>
        <CNav variant="tabs" className="mb-3">
          <CNavItem>
            <CNavLink
              active={activeType === "configmap"}
              onClick={() => setActiveType("configmap")}
            >
              ConfigMaps
            </CNavLink>
          </CNavItem>
          <CNavItem>
            <CNavLink active={activeType === "secret"} onClick={() => setActiveType("secret")}>
              Secrets
            </CNavLink>
          </CNavItem>
        </CNav>
        {statusMessage && <CAlert color={statusVariant}>{statusMessage}</CAlert>}
        {listError && <CAlert color="danger">{listError}</CAlert>}
        <CRow className="g-4">
          <CCol md={4}>
            {loadingList && (
              <div className="d-flex align-items-center gap-2 mb-3">
                <CSpinner size="sm" />
                <span>Loading resources…</span>
              </div>
            )}
            {!loadingList && items.length === 0 && (
              <CAlert color="secondary">No {activeType}s found for this scope.</CAlert>
            )}
            {!loadingList && items.length > 0 && (
              <CListGroup>
                {items.map((item) => {
                  const itemNamespace =
                    item.namespace && item.namespace !== ""
                      ? item.namespace
                      : effectiveNamespace;
                  const isActive =
                    selectedSummary?.name === item.name &&
                    selectedSummary?.namespace === itemNamespace;
                  return (
                    <CListGroupItem
                      key={`${itemNamespace || "cluster"}/${item.name}`}
                      active={isActive}
                      onClick={() => handleSelect({ ...item, namespace: itemNamespace })}
                      role="button"
                    >
                      <div className="fw-semibold">{item.name}</div>
                      <div className="small text-muted">
                        <CBadge color="secondary" className="me-2">
                          {itemNamespace || "default"}
                        </CBadge>
                        {renderMetaBadge(item)}
                      </div>
                    </CListGroupItem>
                  );
                })}
              </CListGroup>
            )}
          </CCol>
          <CCol md={8}>
            {selectedSummary ? (
              <YamlEditorPanel
                title={activeType === "configmap" ? "ConfigMap" : "Secret"}
                resourceType={activeType === "configmap" ? "configmap" : "secret"}
                resourceSummary={selectedSummary}
                resource={resource}
                namespace={selectedSummary.namespace}
                isNew={isNew}
                loading={editorLoading}
                error={editorError}
                onSave={handleSave}
                onDelete={handleDelete}
                onCancel={handleCancel}
              />
            ) : (
              <CAlert color="info">Select a resource to view or edit its YAML.</CAlert>
            )}
          </CCol>
        </CRow>
      </CCardBody>
    </CCard>
  );
};

const renderMetaBadge = (item) => {
  if (item.type_field) {
    return <CBadge color="info">{item.type_field}</CBadge>;
  }
  if (item.data_keys?.length) {
    return <CBadge color="light">{item.data_keys.length} keys</CBadge>;
  }
  return null;
};

const ensureStringMap = (input) => {
  if (!input || typeof input !== "object") {
    return {};
  }
  return Object.entries(input).reduce((acc, [key, value]) => {
    acc[key] = value === undefined || value === null ? "" : String(value);
    return acc;
  }, {});
};

const capitalize = (value = "") =>
  value ? value.charAt(0).toUpperCase() + value.slice(1) : "";

export default ConfigMapsSecretsManager;
