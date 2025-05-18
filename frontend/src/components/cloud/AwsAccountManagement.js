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
  syncAwsAccountResources 
} from "../../services/api";

const AwsAccountManagement = () => {
  const [loading, setLoading] = useState(false);
  const [syncLoading, setSyncLoading] = useState(false);
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
    access_key_id: "",
    secret_access_key: "",
    role_arn: "",
    external_id: "",
    use_role: false
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
    
    // Additional validation for auth method
    if (currentAccount.use_role && !currentAccount.role_arn) {
      setError("IAM Role ARN is required when using role authentication.");
      return;
    }
    
    // When using access key authentication
    if (!currentAccount.use_role) {
      // New account - always require access key ID and secret
      if (!editMode && (!currentAccount.access_key_id || !currentAccount.secret_access_key)) {
        setError("Both Access Key ID and Secret Access Key are required for new accounts.");
        return;
      }
      
      // Existing account - always require access key ID
      if (editMode && !currentAccount.access_key_id) {
        setError("Access Key ID is required when using access key authentication.");
        return;
      }
    }
    
    try {
      setLoading(true);
      setError(null);
      
      if (editMode) {
        // Create a copy of the current account for update
        const accountData = { ...currentAccount };
        
        // Perform some client-side validation for editing
        if (!accountData.use_role && !accountData.access_key_id) {
          setError("Access Key ID is required when using access key authentication.");
          setLoading(false);
          return;
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
  const handleSync = async (accountId) => {
    try {
      setSyncLoading(true);
      setError(null);
      
      const response = await syncAwsAccountResources(accountId);
      setSuccess(`Successfully synced ${response.total_resources || 0} resources from AWS account!`);
      
      // Refresh accounts list to show updated last_synced_at
      fetchAccounts();
    } catch (err) {
      console.error("Error syncing AWS account:", err);
      
      // Provide more specific error messages for common failures
      if (err.response?.data?.error === "CONFIG_ERROR") {
        const message = err.response?.data?.message || "Configuration error";
        
        // Handle profile not found errors with more helpful message
        if (message.includes("AWS configuration not found for profile")) {
          const profileName = message.match(/profile: Some\("([^"]+)"\)/) || 
                            message.match(/profile: "([^"]+)"/) ||
                            ["", "unknown"];
          
          setError(
            <>
              <p><strong>AWS Profile Not Found:</strong> "{profileName[1]}"</p>
              <p>Please check that:</p>
              <ol>
                <li>This profile exists in your AWS credentials file (~/.aws/credentials)</li>
                <li>The profile name is spelled correctly (case-sensitive)</li>
                <li>The application has access to your AWS credentials directory</li>
              </ol>
            </>
          );
        } else {
          setError(`AWS configuration error: ${message}`);
        }
      } else {
        setError("Failed to sync resources from AWS account. Please try again.");
      }
    } finally {
      setSyncLoading(false);
    }
  };
  
  // Handle sync for all accounts
  const handleSyncAll = async () => {
    try {
      setSyncLoading(true);
      setError(null);
      
      // Get all accounts first
      const accounts = await getAwsAccounts();
      
      // Sync each account one by one
      let totalResourcesCount = 0;
      for (const account of accounts) {
        try {
          const response = await syncAwsAccountResources(account.id);
          totalResourcesCount += (response.total_resources || 0);
        } catch (err) {
          console.error(`Error syncing AWS account ${account.account_id}:`, err);
          // Continue with other accounts but keep track of the error
          if (!window.syncErrors) window.syncErrors = [];
          window.syncErrors.push({
            account: account.account_name,
            error: err.response?.data?.message || err.message || "Unknown error"
          });
        }
      }
      
      if (window.syncErrors && window.syncErrors.length > 0) {
        // Create an error message with details of all accounts that failed
        const errorMessages = window.syncErrors.map(e => 
          `• ${e.account}: ${e.error}`
        ).join("\n");
        
        setError(
          <>
            <p><strong>Some accounts failed to sync:</strong></p>
            <pre style={{ whiteSpace: 'pre-wrap', maxHeight: '150px', overflow: 'auto', background: '#f8f9fa', padding: '8px', borderRadius: '4px' }}>
              {errorMessages}
            </pre>
            <p>Successfully synced resources from other accounts.</p>
          </>
        );
        window.syncErrors = []; // Clear errors for next time
      } else {
        setSuccess(`Successfully synced ${totalResourcesCount} resources from all AWS accounts!`);
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
        access_key_id: "",
        secret_access_key: "",
        role_arn: "",
        external_id: "",
        use_role: false
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
        secret_access_key: ""
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
                    <td>
                      {account.use_role ? (
                        <Badge color="info" pill>
                          <i className="fas fa-user-tag me-1"></i>
                          IAM Role
                        </Badge>
                      ) : (
                        <Badge color="warning" pill>
                          <i className="fas fa-key me-1"></i>
                          Access Key
                        </Badge>
                      )}
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
                            handleSync(account.id);
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
            </Row>
            
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
                      <small className="text-muted">
                        ARN of the IAM role to assume
                      </small>
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
    </div>
  );
};

export default AwsAccountManagement;
