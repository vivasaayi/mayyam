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


import React, { useState, useEffect } from "react";
import {
  CCard,
  CCardBody,
  CCardHeader,
  CCol,
  CRow,
  CButton,
  CWidgetStatsA,
} from "@coreui/react";
import { CChart } from "@coreui/react-chartjs";
import { useAuth } from "../hooks/useAuth";

const Dashboard = () => {
  const { user } = useAuth();
  const [stats, setStats] = useState({
    databases: 0,
    kafkaClusters: 0,
    cloudAccounts: 0,
    k8sClusters: 0
  });

  useEffect(() => {
    // In a real app, this would fetch data from the backend
    // For now, we'll just simulate some data
    const fetchDashboardData = async () => {
      // Simulate API call
      setTimeout(() => {
        setStats({
          databases: 5,
          kafkaClusters: 3,
          cloudAccounts: 2,
          k8sClusters: 4
        });
      }, 500);
    };

    fetchDashboardData();
  }, []);

  return (
    <>
      <h2 className="mb-4">Dashboard</h2>
      <CCard className="mb-4">
        <CCardBody>
          <CRow>
            <CCol sm={12}>
              <h4 className="mb-3">Welcome, {user?.username || "User"}!</h4>
              <p className="text-medium-emphasis">
                This is your comprehensive DevOps and SRE toolbox. Use the navigation to access different features.
              </p>
            </CCol>
          </CRow>
        </CCardBody>
      </CCard>

      <CRow>
        <CCol sm={6} lg={3}>
          <CWidgetStatsA
            className="mb-4"
            color="primary"
            value={stats.databases}
            title="Databases"
            action={
              <CButton color="primary" variant="outline" size="sm" className="px-3">
                Manage
              </CButton>
            }
          />
        </CCol>
        <CCol sm={6} lg={3}>
          <CWidgetStatsA
            className="mb-4"
            color="info"
            value={stats.kafkaClusters}
            title="Kafka Clusters"
            action={
              <CButton color="info" variant="outline" size="sm" className="px-3">
                Manage
              </CButton>
            }
          />
        </CCol>
        <CCol sm={6} lg={3}>
          <CWidgetStatsA
            className="mb-4"
            color="warning"
            value={stats.cloudAccounts}
            title="Cloud Accounts"
            action={
              <CButton color="warning" variant="outline" size="sm" className="px-3">
                Manage
              </CButton>
            }
          />
        </CCol>
        <CCol sm={6} lg={3}>
          <CWidgetStatsA
            className="mb-4"
            color="danger"
            value={stats.k8sClusters}
            title="K8s Clusters"
            action={
              <CButton color="danger" variant="outline" size="sm" className="px-3">
                Manage
              </CButton>
            }
          />
        </CCol>
      </CRow>

      <CRow>
        <CCol sm={6}>
          <CCard className="mb-4">
            <CCardHeader>Resource Usage</CCardHeader>
            <CCardBody>
              <CChart
                type="bar"
                data={{
                  labels: ["Database", "Kafka", "Cloud", "Kubernetes"],
                  datasets: [
                    {
                      label: "Resource Count",
                      backgroundColor: ["#3e95cd", "#8e5ea2", "#3cba9f", "#e8c3b9"],
                      data: [
                        stats.databases,
                        stats.kafkaClusters,
                        stats.cloudAccounts,
                        stats.k8sClusters,
                      ],
                    },
                  ],
                }}
                options={{
                  scales: {
                    y: {
                      beginAtZero: true,
                    },
                  },
                }}
              />
            </CCardBody>
          </CCard>
        </CCol>
        <CCol sm={6}>
          <CCard className="mb-4">
            <CCardHeader>Recent Activities</CCardHeader>
            <CCardBody>
              <p>No recent activities</p>
            </CCardBody>
          </CCard>
        </CCol>
      </CRow>
    </>
  );
};

export default Dashboard;
