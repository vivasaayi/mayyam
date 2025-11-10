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


import React, { useState, useEffect, useCallback } from "react";
import {
  Card,
  CardHeader,
  CardBody,
  Button,
  Table,
  Form,
  FormGroup,
  Label,
  Input,
  Row,
  Col,
  Modal,
  ModalHeader,
  ModalBody,
  ModalFooter,
  Alert,
  Badge,
  UncontrolledTooltip
} from "reactstrap";

import Spinner from "../common/Spinner";

import { 
  getAwsAccounts, 
  getAwsAccountById,
  createAwsAccount, 
  updateAwsAccount, 
  deleteAwsAccount, 
  syncAwsAccountResources,
  syncAllAwsAccountResources,
  createSyncRun
} from "../../services/api";
import { listAwsRegions } from "../../services/api";

const AwsAccountManagement = () => {
  const [loading, setLoading] = useState(false);
  const [syncLoading, setSyncLoading] = useState(false);
  const [syncModalOpen, setSyncModalOpen] = useState(false);
  const [syncTargetAccountId, setSyncTargetAccountId] = useState(null);
  const [syncName, setSyncName] = useState("");
  const [syncRegionMode, setSyncRegionMode] = useState("all"); // all | enabled | custom
  const [syncCustomRegions, setSyncCustomRegions] = useState([]);
  const [availableRegions, setAvailableRegions] = useState([]);
  const [accounts, setAccounts] = useState([]);
  const [error, setError] = useState(null);
  const [success, setSuccess] = useState(null);
  const [modalOpen, setModalOpen] = useState(false);
  const [editMode, setEditMode] = useState(false);
  const [currentAccount, setCurrentAccount] = useState({
    account_id: "",
    account_name: "",
    profile: "",
    default_region: "",
    regions: [],
    access_key_id: "",
    secret_access_key: "",
    role_arn: "",
    external_id: "",
    use_role: false,
    // New auth fields
    auth_type: "auto", // auto | profile | sso | assume_role | web_identity | instance_role | access_keys
    source_profile: "",
    sso_profile: "",
    web_identity_token_file: "",
    session_name: "",
  });
  
  // Fetch all accounts on component mount
  useEffect(() => {
    fetchAccounts();
  }, []);
  
  // Fetch accounts from backend
  const fetchAccounts = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      
      const accounts = await getAwsAccounts();
      setAccounts(accounts);
    } catch (err) {
      console.error("Error fetching AWS accounts:", err);
      setError("Failed to fetch AWS accounts. Please try again.");
    } finally {
      setLoading(false);
    }
  }, []);

  // Handle form submission
  const handleSubmit = async (e) => {
    e.preventDefault();
    
    // Validate the form
    if (!currentAccount.account_id || !currentAccount.account_name || !currentAccount.default_region) {
      setError("Please fill in all required fields.");
      return;
    }
    
    // Validation based on auth_type
    const auth = (currentAccount.auth_type || 'auto').toLowerCase();
    if (auth === 'access_keys') {
      if (!currentAccount.access_key_id || (!editMode && !currentAccount.secret_access_key)) {
        setError("Access Keys auth requires Access Key ID and Secret (secret can be left blank on edit to keep existing).");
        return;
      }
    } else if (auth === 'assume_role') {
      if (!currentAccount.role_arn) {
        setError("Assume Role auth requires a Role ARN.");
        return;
      }
    } else if (auth === 'web_identity') {
      if (!currentAccount.role_arn) {
        setError("Web Identity auth requires a Role ARN.");
        return;
      }
      // token file optional; session name optional
    } else if (auth === 'profile') {
      if (!currentAccount.profile) {
        setError("Profile auth requires a profile name.");
        return;
      }
    } else if (auth === 'sso') {
      if (!currentAccount.sso_profile && !currentAccount.profile) {
        setError("SSO auth requires sso_profile or profile.");
        return;
      }
    } else if (auth === 'auto') {
      // Back-compat: use legacy flags
      if (currentAccount.use_role && !currentAccount.role_arn) {
        setError("IAM Role ARN is required when using role authentication.");
        return;
      }
      if (!currentAccount.use_role) {
        if (!editMode && (!currentAccount.access_key_id || !currentAccount.secret_access_key)) {
          setError("Both Access Key ID and Secret Access Key are required for new accounts.");
          return;
        }
        if (editMode && !currentAccount.access_key_id) {
          setError("Access Key ID is required when using access key authentication.");
          return;
        }
      }
    }
    
    try {
      setLoading(true);
      setError(null);
      
      if (editMode) {
        // Create a copy of the current account for update
        const accountData = { ...currentAccount };
        
        // Perform some client-side validation for editing
        // Additional guard for legacy auto path
        if ((accountData.auth_type || 'auto') === 'auto') {
          if (!accountData.use_role && !accountData.access_key_id) {
            setError("Access Key ID is required when using access key authentication.");
            setLoading(false);
            return;
          }
        }
        
        // If the secret key is empty in edit mode, remove it from the request
        // to keep the existing one in the database
        if (!accountData.secret_access_key) {
          delete accountData.secret_access_key;
        }
        
        console.log("Updating account with data:", accountData);
        
        await updateAwsAccount(currentAccount.id, accountData);
        setSuccess("AWS account updated successfully!");
      } else {
        await createAwsAccount(currentAccount);
        setSuccess("AWS account added successfully!");
      }
      
      // Refresh accounts list
      fetchAccounts();
      
      // Close modal and reset form
      toggleModal();
    } catch (err) {
      console.error("Error saving AWS account:", err);
      setError(err.response?.data?.message || "Failed to save AWS account. Please check your inputs and try again.");
    } finally {
      setLoading(false);
    }
  };

  // Handle account deletion
  const handleDelete = async (accountId) => {
    const account = accounts.find(acc => acc.id === accountId);
    if (!account) return;
    
    // Improved confirmation dialog with specific account details
    if (!window.confirm(`Delete AWS account "${account.account_name}" (${account.account_id})?\n\nThis action cannot be undone and all associated resource data will be removed from Mayyam. Your actual AWS account and resources will not be affected.`)) {
      return;
    }
    
    try {
      setLoading(true);
      setError(null);
      
      await deleteAwsAccount(accountId);
      setSuccess(`AWS account "${account.account_name}" deleted successfully!`);
      
      // Refresh accounts list
      fetchAccounts();
    } catch (err) {
      console.error("Error deleting AWS account:", err);
      setError(`Failed to delete AWS account "${account.account_name}". Please try again.`);
    } finally {
      setLoading(false);
    }
  };
  
  // Handle sync for a specific account
  const openSyncModal = (accountId) => {
    setSyncTargetAccountId(accountId);
    setSyncName("");
    setSyncRegionMode("all");
    setSyncCustomRegions([]);
    setSyncModalOpen(true);
    // Preload regions for better UX
    const acct = accounts.find(a => a.id === accountId);
    listAwsRegions({ accountId, profile: acct?.profile || null, region: acct?.default_region || null })
      .then(setAvailableRegions)
      .catch(() => setAvailableRegions([]));
  };

  const closeSyncModal = () => {
    setSyncModalOpen(false);
    setSyncTargetAccountId(null);
    setSyncName("");
  };

  const confirmSync = async () => {
    if (!syncTargetAccountId) return;
    if (!syncName || syncName.trim() === "") {
      setError("Please provide a name for this sync run.");
      return;
    }

    try {
      setSyncLoading(true);
      setError(null);

      // 1) Create a sync run (server will generate UUID)
      // Build metadata for region selection
      const metadata = {};
      if (syncRegionMode === "all") {
        metadata.all_regions = true;
      } else if (syncRegionMode === "enabled") {
        // Resolve enabled regions for this account
        const acct = accounts.find(a => a.id === syncTargetAccountId);
        if (acct?.regions && acct.regions.length > 0) {
          metadata.regions = acct.regions;
        } else {
          metadata.all_regions = true; // fallback to all if none configured
        }
      } else if (syncRegionMode === "custom") {
        if (syncCustomRegions.length > 0) {
          metadata.regions = syncCustomRegions;
        } else {
          metadata.all_regions = true;
        }
      }

      const run = await createSyncRun({ name: syncName, aws_account_id: syncTargetAccountId, metadata });
      const syncId = run.id;

      // 2) Trigger account sync with the sync_id
      const response = await syncAwsAccountResources(syncTargetAccountId, syncId);
      setSuccess(`Started sync '${syncName}' (ID: ${syncId}). ${response.count || 0} resources processed.`);

      // Refresh accounts list to show updated last_synced_at
      fetchAccounts();
    } catch (err) {
      console.error("Error starting sync run:", err);
      setError(err.response?.data?.message || "Failed to start sync. Please try again.");
    } finally {
      setSyncLoading(false);
      closeSyncModal();
    }
  };
  
  // Handle sync for all accounts
  const handleSyncAll = async () => {
    try {
      setSyncLoading(true);
      setError(null);
      
      // Use the new endpoint to sync all accounts in a single request
      const response = await syncAllAwsAccountResources();
      
      if (response.success) {
        setSuccess(`Successfully synced ${response.count} resources from all AWS accounts!`);
      } else {
        // Handle partial success case
        setError(
          <>
            <p><strong>Some accounts failed to sync:</strong></p>
            <p>{response.message}</p>
          </>
        );
      }
      
      // Refresh accounts list to show updated last_synced_at
      fetchAccounts();
    } catch (err) {
      console.error("Error syncing all AWS accounts:", err);
      setError("Failed to sync resources from AWS accounts. Please try again.");
    } finally {
      setSyncLoading(false);
    }
  };

  // Toggle modal and reset form
  const toggleModal = () => {
    setModalOpen(!modalOpen);
    if (!modalOpen) {
      setEditMode(false);
      setCurrentAccount({
        account_id: "",
        account_name: "",
        profile: "",
        default_region: "",
        regions: [],
        access_key_id: "",
        secret_access_key: "",
        role_arn: "",
        external_id: "",
        use_role: false,
        auth_type: "auto",
        source_profile: "",
        sso_profile: "",
        web_identity_token_file: "",
        session_name: "",
      });
    }
  };
  
  // Edit account
  const handleEdit = async (account) => {
    try {
      setLoading(true);
      
      // Fetch the complete account details from the backend
      const fullAccount = await getAwsAccountById(account.id);
      
      console.log("Fetched account details:", fullAccount);
      
      // Create a copy of the account object to avoid modifying the original
      const accountCopy = {
        ...fullAccount,
        // Keep the access_key_id from the backend if available, otherwise fallback
        access_key_id: fullAccount.access_key_id || "",
        // Clear the secret key since we don't want to show it in the form
        // The backend will keep the existing one if it's left blank
        secret_access_key: "",
        // Normalize new fields for controlled inputs
        auth_type: fullAccount.auth_type || 'auto',
        source_profile: fullAccount.source_profile || "",
        sso_profile: fullAccount.sso_profile || "",
        web_identity_token_file: fullAccount.web_identity_token_file || "",
        session_name: fullAccount.session_name || "",
      };
      
      setEditMode(true);
      setCurrentAccount(accountCopy);
      setModalOpen(true);
    } catch (err) {
      console.error("Error fetching account details:", err);
      setError("Failed to load account details for editing. Please try again.");
    } finally {
      setLoading(false);
    }
  };
  
  // Handle input change
  const handleInputChange = (e) => {
    const { name, value, type, checked } = e.target;
    setCurrentAccount({
      ...currentAccount,
      [name]: type === "checkbox" ? checked : value
    });
  };
  
  // Clear all alerts
  const clearAlerts = () => {
    setError(null);
    setSuccess(null);
  };

  return (
    <div>
      {error && (
        <Alert color="danger" toggle={clearAlerts}>
          <i className="fas fa-exclamation-circle me-2"></i>
          {error}
        </Alert>
      )}
      
      {success && (
        <Alert color="success" toggle={clearAlerts}>
          <i className="fas fa-check-circle me-2"></i>
          {success}
        </Alert>
      )}
      
      <Card>
        <CardHeader className="d-flex justify-content-between align-items-center">
          <h5 className="mb-0">
            <i className="fas fa-key me-2"></i>
            AWS Account Management
          </h5>
          <div>
            <Button 
              color="success" 
              size="sm" 
              className="me-2"
              onClick={handleSyncAll}
              disabled={syncLoading || accounts.length === 0}
            >
              {syncLoading ? (
                <>
                  <Spinner size="sm" className="me-1" />
                  Syncing...
                </>
              ) : (
                <>
                  <i className="fas fa-sync-alt me-1"></i>
                  Sync All Accounts
                </>
              )}
            </Button>
            
            <Button color="primary" size="sm" onClick={toggleModal}>
              <i className="fas fa-plus me-1"></i>
              Add Account
            </Button>
          </div>
        </CardHeader>
        
        <CardBody>
          {loading ? (
            <div className="text-center p-5">
              <Spinner size="lg" />
              <p className="mt-3">Loading AWS accounts...</p>
            </div>
          ) : accounts.length === 0 ? (
            <div className="text-center p-5">
              <i className="fas fa-cloud fa-3x text-muted mb-3"></i>
              <h4>No AWS Accounts Found</h4>
              <p className="text-muted mb-4">
                You haven't added any AWS accounts yet. Add an account to start syncing and managing resources.
              </p>
              <Button color="primary" onClick={toggleModal}>
                <i className="fas fa-plus me-1"></i>
                Add AWS Account
              </Button>
            </div>
          ) : (
            <Table responsive striped hover>
              <thead>
                <tr>
                  <th>Account ID</th>
                  <th>Name</th>
                  <th>Profile</th>
                  <th>Default Region</th>
                  <th>Regions</th>
                  <th>Auth Method</th>
                  <th>Last Sync</th>
                  <th>Actions</th>
                </tr>
              </thead>
              <tbody>
                {accounts.map((account) => (
                  <tr key={account.id}>
                    <td>
                      <div className="d-flex align-items-center">
                        <Badge 
                          color="light" 
                          className="me-2" 
                          pill
                          id={`account-${account.id}`}
                        >
                          <i className="fab fa-aws"></i>
                        </Badge>
                        <UncontrolledTooltip target={`account-${account.id}`}>
                          AWS Account
                        </UncontrolledTooltip>
                        {account.account_id}
                      </div>
                    </td>
                    <td>{account.account_name}</td>
                    <td>{account.profile || "N/A"}</td>
                    <td>{account.default_region}</td>
                    <td>{account.regions && account.regions.length > 0 ? account.regions.join(", ") : "All"}</td>
                    <td>
                      {(() => {
                        const t = (account.auth_type || (account.use_role ? 'assume_role' : 'access_keys')).toLowerCase();
                        const map = {
                          access_keys: { color: 'warning', icon: 'fa-key', label: 'Access Keys' },
                          assume_role: { color: 'info', icon: 'fa-user-tag', label: 'Assume Role' },
                          web_identity: { color: 'info', icon: 'fa-id-badge', label: 'Web Identity' },
                          sso: { color: 'primary', icon: 'fa-sitemap', label: 'SSO' },
                          profile: { color: 'secondary', icon: 'fa-user-cog', label: 'Profile' },
                          instance_role: { color: 'success', icon: 'fa-server', label: 'Instance/Task Role' },
                          auto: { color: 'dark', icon: 'fa-magic', label: 'Auto' },
                        };
                        const m = map[t] || map['auto'];
                        return (
                          <Badge color={m.color} pill>
                            <i className={`fas ${m.icon} me-1`}></i>
                            {m.label}
                          </Badge>
                        );
                      })()}
                    </td>
                    <td>
                      {account.last_synced_at ? (
                        <span>{new Date(account.last_synced_at).toLocaleString()}</span>
                      ) : (
                        <Badge color="light" pill>Never</Badge>
                      )}
                    </td>
                    <td>
                      <div className="d-flex gap-2">
                        {/* Text-based action links for better visibility */}
                        <a 
                          href="#" 
                          className="text-primary"
                          onClick={(e) => {
                            e.preventDefault();
                            openSyncModal(account.id);
                          }}
                          style={{ textDecoration: 'none', fontWeight: 'bold' }}
                          id={`sync-${account.id}`}
                        >
                          Sync
                        </a>
                        <UncontrolledTooltip target={`sync-${account.id}`}>
                          Sync resources from this account
                        </UncontrolledTooltip>
                        
                        <a 
                          href="#" 
                          className="text-secondary"
                          onClick={(e) => {
                            e.preventDefault();
                            handleEdit(account);
                          }}
                          style={{ textDecoration: 'none', fontWeight: 'bold' }}
                          id={`edit-${account.id}`}
                        >
                          Edit
                        </a>
                        <UncontrolledTooltip target={`edit-${account.id}`}>
                          Edit account details
                        </UncontrolledTooltip>
                        
                        <a 
                          href="#" 
                          className="text-danger"
                          onClick={(e) => {
                            e.preventDefault();
                            handleDelete(account.id);
                          }}
                          style={{ textDecoration: 'none', fontWeight: 'bold' }}
                          id={`delete-${account.id}`}
                        >
                          Delete
                        </a>
                        <UncontrolledTooltip target={`delete-${account.id}`}>
                          Delete this account
                        </UncontrolledTooltip>
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </Table>
          )}
        </CardBody>
      </Card>
      
      {/* Add/Edit Account Modal */}
      <Modal isOpen={modalOpen} toggle={toggleModal} size="lg">
        <ModalHeader toggle={toggleModal}>
          {editMode ? "Edit AWS Account" : "Add New AWS Account"}
        </ModalHeader>
        
        <ModalBody>
          <Form onSubmit={handleSubmit}>
            <Row>
              <Col md={6}>
                <FormGroup>
                  <Label for="account_id">AWS Account ID*</Label>
                  <Input
                    type="text"
                    name="account_id"
                    id="account_id"
                    placeholder="e.g., 123456789012"
                    value={currentAccount.account_id}
                    onChange={handleInputChange}
                    required
                  />
                  <small className="text-muted">
                    Your 12-digit AWS account identifier
                  </small>
                </FormGroup>
              </Col>
              
              <Col md={6}>
                <FormGroup>
                  <Label for="account_name">Account Name*</Label>
                  <Input
                    type="text"
                    name="account_name"
                    id="account_name"
                    placeholder="e.g., Production"
                    value={currentAccount.account_name}
                    onChange={handleInputChange}
                    required
                  />
                  <small className="text-muted">
                    A friendly name for this account
                  </small>
                </FormGroup>
              </Col>
            </Row>
            
            <Row>
              <Col md={6}>
                <FormGroup>
                  <Label for="profile">AWS Profile</Label>
                  <Input
                    type="text"
                    name="profile"
                    id="profile"
                    placeholder="e.g., default"
                    value={currentAccount.profile}
                    onChange={handleInputChange}
                  />
                  <small className="text-muted">
                    Optional profile name from AWS credentials file (~/.aws/credentials). 
                    Must exactly match a profile in your credentials file (case-sensitive).
                  </small>
                </FormGroup>
              </Col>
              
              <Col md={6}>
                <FormGroup>
                  <Label for="default_region">Default Region*</Label>
                  <Input
                    type="text"
                    name="default_region"
                    id="default_region"
                    placeholder="e.g., us-west-2"
                    value={currentAccount.default_region}
                    onChange={handleInputChange}
                    required
                  />
                  <small className="text-muted">
                    Primary AWS region for this account
                  </small>
                </FormGroup>
              </Col>
              
              <Col md={6}>
                <FormGroup>
                  <Label for="regions">Enabled Regions</Label>
                  <Input
                    type="select"
                    name="regions"
                    id="regions"
                    multiple
                    value={currentAccount.regions}
                    onChange={(e) => {
                      const selected = Array.from(e.target.selectedOptions, option => option.value);
                      setCurrentAccount(prev => ({ ...prev, regions: selected }));
                    }}
                  >
                    <option value="us-east-1">US East (N. Virginia)</option>
                    <option value="us-east-2">US East (Ohio)</option>
                    <option value="us-west-1">US West (N. California)</option>
                    <option value="us-west-2">US West (Oregon)</option>
                    <option value="eu-west-1">EU (Ireland)</option>
                    <option value="eu-central-1">EU (Frankfurt)</option>
                    <option value="ap-southeast-1">Asia Pacific (Singapore)</option>
                    <option value="ap-northeast-1">Asia Pacific (Tokyo)</option>
                    <option value="ap-south-1">Asia Pacific (Mumbai)</option>
                    <option value="ca-central-1">Canada (Central)</option>
                    <option value="sa-east-1">South America (São Paulo)</option>
                  </Input>
                  <small className="text-muted">
                    Select regions to monitor for this account (leave empty for all)
                  </small>
                </FormGroup>
              </Col>
            </Row>
            
            <FormGroup className="mb-4">
              <Label for="auth_type">Authentication Method</Label>
              <Input
                type="select"
                name="auth_type"
                id="auth_type"
                value={currentAccount.auth_type}
                onChange={handleInputChange}
              >
                <option value="auto">Auto (recommended)</option>
                <option value="profile">Profile</option>
                <option value="sso">SSO (via profile)</option>
                <option value="assume_role">Assume Role</option>
                <option value="web_identity">Web Identity (OIDC)</option>
                <option value="instance_role">Instance/Task/Pod Role</option>
                <option value="access_keys">Access Keys</option>
              </Input>
              <small className="text-muted">
                Choose how Mayyam authenticates to this AWS account. "Auto" preserves the legacy behavior using the fields below.
              </small>
            </FormGroup>
            
            {/* Conditional inputs based on auth_type */}
            {currentAccount.auth_type === 'assume_role' ? (
              <div className="role-auth-section">
                <Row>
                  <Col md={6}>
                    <FormGroup>
                      <Label for="role_arn">IAM Role ARN*</Label>
                      <Input
                        type="text"
                        name="role_arn"
                        id="role_arn"
                        placeholder="e.g., arn:aws:iam::123456789012:role/MayyamRole"
                        value={currentAccount.role_arn}
                        onChange={handleInputChange}
                        required={true}
                      />
                      <small className="text-muted">
                        ARN of the IAM role to assume
                      </small>
                    </FormGroup>
                  </Col>
                  
                  <Col md={6}>
                    <FormGroup>
                      <Label for="source_profile">Source Profile</Label>
                      <Input
                        type="text"
                        name="source_profile"
                        id="source_profile"
                        placeholder="Profile to source base credentials"
                        value={currentAccount.source_profile}
                        onChange={handleInputChange}
                      />
                      <small className="text-muted">Optional profile that provides the credentials to call STS AssumeRole.</small>
                    </FormGroup>
                  </Col>

                  <Col md={6}>
                    <FormGroup>
                      <Label for="external_id">External ID</Label>
                      <Input
                        type="text"
                        name="external_id"
                        id="external_id"
                        placeholder="External ID for role assumption"
                        value={currentAccount.external_id}
                        onChange={handleInputChange}
                      />
                      <small className="text-muted">
                        Optional external ID for enhanced security
                      </small>
                    </FormGroup>
                  </Col>

                  <Col md={6}>
                    <FormGroup>
                      <Label for="session_name">Session Name</Label>
                      <Input
                        type="text"
                        name="session_name"
                        id="session_name"
                        placeholder="Session name for the assumed role"
                        value={currentAccount.session_name}
                        onChange={handleInputChange}
                      />
                      <small className="text-muted">Optional STS session name.</small>
                    </FormGroup>
                  </Col>
                </Row>
              </div>
            ) : currentAccount.auth_type === 'web_identity' ? (
              <div className="role-auth-section">
                <Row>
                  <Col md={6}>
                    <FormGroup>
                      <Label for="role_arn">IAM Role ARN*</Label>
                      <Input
                        type="text"
                        name="role_arn"
                        id="role_arn"
                        placeholder="e.g., arn:aws:iam::123456789012:role/MayyamWebIdentityRole"
                        value={currentAccount.role_arn}
                        onChange={handleInputChange}
                        required={true}
                      />
                      <small className="text-muted">Role to assume via web identity.</small>
                    </FormGroup>
                  </Col>
                  <Col md={6}>
                    <FormGroup>
                      <Label for="web_identity_token_file">Token File</Label>
                      <Input
                        type="text"
                        name="web_identity_token_file"
                        id="web_identity_token_file"
                        placeholder="Path to OIDC token file (optional)"
                        value={currentAccount.web_identity_token_file}
                        onChange={handleInputChange}
                      />
                      <small className="text-muted">Optional. If omitted, SDK may read from environment.</small>
                    </FormGroup>
                  </Col>
                  <Col md={6}>
                    <FormGroup>
                      <Label for="session_name">Session Name</Label>
                      <Input
                        type="text"
                        name="session_name"
                        id="session_name"
                        placeholder="Session name for the assumed role"
                        value={currentAccount.session_name}
                        onChange={handleInputChange}
                      />
                    </FormGroup>
                  </Col>
                </Row>
              </div>
            ) : currentAccount.auth_type === 'sso' ? (
              <div className="profile-auth-section">
                <Row>
                  <Col md={6}>
                    <FormGroup>
                      <Label for="sso_profile">SSO Profile</Label>
                      <Input
                        type="text"
                        name="sso_profile"
                        id="sso_profile"
                        placeholder="Profile configured for SSO"
                        value={currentAccount.sso_profile}
                        onChange={handleInputChange}
                      />
                      <small className="text-muted">Use an AWS profile that is configured for SSO (in ~/.aws/config).</small>
                    </FormGroup>
                  </Col>
                  <Col md={6}>
                    <FormGroup>
                      <Label for="profile">Profile (alternate)</Label>
                      <Input
                        type="text"
                        name="profile"
                        id="profile"
                        placeholder="e.g., default"
                        value={currentAccount.profile}
                        onChange={handleInputChange}
                      />
                    </FormGroup>
                  </Col>
                </Row>
              </div>
            ) : currentAccount.auth_type === 'profile' ? (
              <div className="profile-auth-section">
                <Row>
                  <Col md={6}>
                    <FormGroup>
                      <Label for="profile">AWS Profile*</Label>
                      <Input
                        type="text"
                        name="profile"
                        id="profile"
                        placeholder="e.g., default"
                        value={currentAccount.profile}
                        onChange={handleInputChange}
                        required
                      />
                      <small className="text-muted">Profile name from ~/.aws/credentials or ~/.aws/config.</small>
                    </FormGroup>
                  </Col>
                </Row>
              </div>
            ) : currentAccount.auth_type === 'instance_role' ? (
              <Alert color="info">
                Using the default credential provider chain. On AWS, this resolves to the instance/task/IRSA role.
              </Alert>
            ) : currentAccount.auth_type === 'access_keys' ? (
              <div className="key-auth-section">
                <Row>
                  <Col md={6}>
                    <FormGroup>
                      <Label for="access_key_id">Access Key ID*</Label>
                      <Input
                        type="text"
                        name="access_key_id"
                        id="access_key_id"
                        placeholder="e.g., AKIAIOSFODNN7EXAMPLE"
                        value={currentAccount.access_key_id}
                        onChange={handleInputChange}
                        required={currentAccount.auth_type === 'access_keys'}
                      />
                    </FormGroup>
                  </Col>
                  
                  <Col md={6}>
                    <FormGroup>
                      <Label for="secret_access_key">Secret Access Key*</Label>
                      <Input
                        type="password"
                        name="secret_access_key"
                        id="secret_access_key"
                        placeholder={editMode ? "••••••••••••••••" : "Enter secret access key"}
                        value={currentAccount.secret_access_key}
                        onChange={handleInputChange}
                        required={currentAccount.auth_type === 'access_keys' && !editMode}
                      />
                      {editMode && (
                        <small className="text-muted">
                          Leave blank to keep the existing secret key
                        </small>
                      )}
                    </FormGroup>
                  </Col>
                </Row>
                <Alert color="warning">
                  <i className="fas fa-exclamation-triangle me-2"></i>
                  Access keys are stored securely, but using IAM roles is recommended for better security.
                </Alert>
              </div>
            ) : (
              // Auto (legacy): offer the legacy toggle/fields for back-compat
              <>
                <FormGroup className="mb-4">
                  <div className="form-check">
                    <Input
                      type="checkbox"
                      className="form-check-input"
                      id="use_role"
                      name="use_role"
                      checked={currentAccount.use_role}
                      onChange={handleInputChange}
                    />
                    <Label className="form-check-label" for="use_role">
                      Use IAM Role for authentication (recommended)
                    </Label>
                  </div>
                  <small className="text-muted">
                    When enabled, uses IAM Role assumption instead of access keys
                  </small>
                </FormGroup>
                {currentAccount.use_role ? (
                  <div className="role-auth-section">
                    <Row>
                      <Col md={6}>
                        <FormGroup>
                          <Label for="role_arn">IAM Role ARN*</Label>
                          <Input
                            type="text"
                            name="role_arn"
                            id="role_arn"
                            placeholder="e.g., arn:aws:iam::123456789012:role/MayyamRole"
                            value={currentAccount.role_arn}
                            onChange={handleInputChange}
                            required={currentAccount.use_role}
                          />
                          <small className="text-muted">ARN of the IAM role to assume</small>
                        </FormGroup>
                      </Col>
                      <Col md={6}>
                        <FormGroup>
                          <Label for="external_id">External ID</Label>
                          <Input
                            type="text"
                            name="external_id"
                            id="external_id"
                            placeholder="External ID for role assumption"
                            value={currentAccount.external_id}
                            onChange={handleInputChange}
                          />
                        </FormGroup>
                      </Col>
                    </Row>
                  </div>
                ) : (
                  <div className="key-auth-section">
                    <Row>
                      <Col md={6}>
                        <FormGroup>
                          <Label for="access_key_id">Access Key ID*</Label>
                          <Input
                            type="text"
                            name="access_key_id"
                            id="access_key_id"
                            placeholder="e.g., AKIAIOSFODNN7EXAMPLE"
                            value={currentAccount.access_key_id}
                            onChange={handleInputChange}
                            required={!currentAccount.use_role}
                          />
                        </FormGroup>
                      </Col>
                      <Col md={6}>
                        <FormGroup>
                          <Label for="secret_access_key">Secret Access Key*</Label>
                          <Input
                            type="password"
                            name="secret_access_key"
                            id="secret_access_key"
                            placeholder={editMode ? "••••••••••••••••" : "Enter secret access key"}
                            value={currentAccount.secret_access_key}
                            onChange={handleInputChange}
                            required={!currentAccount.use_role && !editMode}
                          />
                          {editMode && (
                            <small className="text-muted">Leave blank to keep the existing secret key</small>
                          )}
                        </FormGroup>
                      </Col>
                    </Row>
                    <Alert color="warning">
                      <i className="fas fa-exclamation-triangle me-2"></i>
                      Access keys are stored securely, but using IAM roles is recommended for better security.
                    </Alert>
                  </div>
                )}
              </>
            )}
          </Form>
        </ModalBody>
        
        <ModalFooter>
          <Button color="secondary" onClick={toggleModal}>
            Cancel
          </Button>
          <Button 
            color="primary" 
            onClick={handleSubmit}
            disabled={loading}
          >
            {loading ? (
              <>
                <Spinner size="sm" className="me-1" />
                Saving...
              </>
            ) : (
              <>
                <i className="fas fa-save me-1"></i>
                {editMode ? "Update" : "Add"} Account
              </>
            )}
          </Button>
        </ModalFooter>
      </Modal>

      {/* Sync Run Modal */}
      <Modal isOpen={syncModalOpen} toggle={closeSyncModal}>
        <ModalHeader toggle={closeSyncModal}>Start Sync</ModalHeader>
        <ModalBody>
          <Form onSubmit={(e) => { e.preventDefault(); confirmSync(); }}>
            <FormGroup>
              <Label for="sync_name">Sync Name</Label>
              <Input
                type="text"
                name="sync_name"
                id="sync_name"
                placeholder="e.g., Nightly snapshot, Investigate Kinesis, etc."
                value={syncName}
                onChange={(e) => setSyncName(e.target.value)}
                required
              />
              <small className="text-muted">Give this sync a name to track it later.</small>
            </FormGroup>

            {/* Region selection */}
            <FormGroup>
              <Label>Regions to Scan</Label>
              <div className="d-flex gap-3 align-items-center flex-wrap">
                <div className="form-check">
                  <input className="form-check-input" type="radio" name="syncRegionMode" id="regionAll" value="all"
                    checked={syncRegionMode === 'all'} onChange={() => setSyncRegionMode('all')} />
                  <Label className="form-check-label" htmlFor="regionAll">All regions (default)</Label>
                </div>
                <div className="form-check">
                  <input className="form-check-input" type="radio" name="syncRegionMode" id="regionEnabled" value="enabled"
                    checked={syncRegionMode === 'enabled'} onChange={() => setSyncRegionMode('enabled')} />
                  <Label className="form-check-label" htmlFor="regionEnabled">Account Enabled regions</Label>
                </div>
                <div className="form-check">
                  <input className="form-check-input" type="radio" name="syncRegionMode" id="regionCustom" value="custom"
                    checked={syncRegionMode === 'custom'} onChange={() => setSyncRegionMode('custom')} />
                  <Label className="form-check-label" htmlFor="regionCustom">Custom selection</Label>
                </div>
              </div>
            </FormGroup>

            {syncRegionMode === 'enabled' && (() => {
              const acct = accounts.find(a => a.id === syncTargetAccountId);
              if (!acct) return null;
              if (!acct.regions || acct.regions.length === 0) {
                return (
                  <Alert color="info">
                    This account has no enabled regions configured. Using this option will fall back to All regions.
                    Go to Edit Account to set "Enabled Regions".
                  </Alert>
                );
              }
              return null;
            })()}

            {syncRegionMode === 'custom' && (
              <FormGroup>
                <Label>Select Regions</Label>
                <Input
                  type="select"
                  multiple
                  value={syncCustomRegions}
                  onChange={(e) => {
                    const opts = Array.from(e.target.selectedOptions).map(o => o.value);
                    setSyncCustomRegions(opts);
                  }}
                >
                  {availableRegions.length > 0 ? (
                    availableRegions.map(r => (
                      <option key={r} value={r}>{r}</option>
                    ))
                  ) : (
                    <option value="">Loading regions...</option>
                  )}
                </Input>
                <small className="text-muted">Hold Cmd/Ctrl to select multiple regions.</small>
              </FormGroup>
            )}
          </Form>
        </ModalBody>
        <ModalFooter>
          <Button color="secondary" onClick={closeSyncModal}>Cancel</Button>
          <Button color="primary" onClick={confirmSync} disabled={syncLoading}>
            {syncLoading ? <Spinner size="sm" className="me-1" /> : <i className="fas fa-play me-1"></i>}
            Start Sync
          </Button>
        </ModalFooter>
      </Modal>
    </div>
  );
};

export default AwsAccountManagement;
