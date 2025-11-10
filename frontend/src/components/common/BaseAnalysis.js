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


import React, { useEffect, useRef } from "react";
import { 
  Card, CardBody, Button, Spinner, Alert, Badge
} from "reactstrap";
import QuestionHistory from './QuestionHistory';
import FiveWhyProgress from './FiveWhyProgress';
import RelatedQuestionsPanel from './RelatedQuestionsPanel';
import FiveWhySummary from './FiveWhySummary';
// Using react-icons for icon components - ensure react-icons is installed
import { FaQuestionCircle, FaSpinner } from 'react-icons/fa';

/**
 * Custom hook to get the previous value of a prop or state.
 */
function usePrevious(value) {
  const ref = useRef();
  useEffect(() => {
    ref.current = value;
  }, [value]);
  return ref.current;
}

const BaseAnalysis = ({
  title,
  resource,
  workflows, // Array of available workflow objects { id, name, description }
  onRunAnalysis, // Function to call to run the initial analysis of a selected workflow (workflowId) => {}
  result, // The latest analysis result object from the parent
  loading, // General loading state from the parent (e.g., while resource or workflows are loading)
  error, // Error object from parent
  selectedWorkflow, // ID of the currently selected workflow
  onAskQuestion, // Function to call when a follow-up question is asked (questionText, workflowIdContext) => {}
  initialQuestionFromUrl, // An initial question passed via URL query params
}) => {
  // Combined state for the entire Q&A history
  // Each item: { questionText: string, answer: object | null, isProcessing: boolean, isInitial: boolean }
  const [qaHistory, setQaHistory] = React.useState([]);
  const [currentAnalysisDepth, setCurrentAnalysisDepth] = React.useState(0); // 1-based depth, number of answered questions
  
  const prevSelectedWorkflow = usePrevious(selectedWorkflow);
  const prevInitialQuestionFromUrl = usePrevious(initialQuestionFromUrl);
  const prevResult = usePrevious(result);
  // No need to track previous loading state since we're using other conditions for flow control

  // Effect to reset Q&A history when the primary analysis driver (workflow or URL question) changes.
  React.useEffect(() => {
    if (selectedWorkflow !== prevSelectedWorkflow || initialQuestionFromUrl !== prevInitialQuestionFromUrl) {
      console.log(
        `BaseAnalysis: Workflow or initial URL question changed. New Workflow: ${selectedWorkflow}, New URL Question: ${initialQuestionFromUrl}. Resetting Q&A history.`
      );
      setQaHistory([]);
      setCurrentAnalysisDepth(0);
    }
  }, [selectedWorkflow, initialQuestionFromUrl, prevSelectedWorkflow, prevInitialQuestionFromUrl]);

  // Effect to trigger the initial analysis (either from URL question or selected workflow)
  React.useEffect(() => {
    // Only proceed if history is empty and the parent isn't in a general loading state (e.g. loading resource details).
    // The `loading` here refers to the prop passed from the parent, which might indicate app-level loading,
    // not specific to an analysis call initiated by this component.
    if (qaHistory.length === 0 && !loading) {
      if (initialQuestionFromUrl && onAskQuestion) {
        console.log("BaseAnalysis: Triggering analysis for initial question from URL:", initialQuestionFromUrl);
        setQaHistory([{
          questionText: initialQuestionFromUrl,
          answer: null,
          isProcessing: true,
          isInitial: true, // Treat as the first step of this analysis session
        }]);
        // setCurrentAnalysisDepth(0); // Depth updates upon receiving answer
        onAskQuestion(initialQuestionFromUrl, selectedWorkflow || null); // Pass current workflow as context if any
      } else if (selectedWorkflow && onRunAnalysis) {
        const workflow = workflows.find(w => w.id === selectedWorkflow);
        if (workflow) {
          console.log("BaseAnalysis: Triggering initial run for selected workflow:", workflow.name);
          setQaHistory([{
            questionText: workflow.name, // Use workflow name as the "question" for initial workflow step
            answer: null,
            isProcessing: true,
            isInitial: true,
          }]);
          // setCurrentAnalysisDepth(0); // Depth updates upon receiving answer
          onRunAnalysis(selectedWorkflow);
        }
      }
    }
  }, [
    initialQuestionFromUrl,
    selectedWorkflow,
    qaHistory.length, // Re-evaluate if history is cleared
    loading,          // Parent's general loading state
    onAskQuestion,
    onRunAnalysis,
    workflows,
  ]);

  // Effect to process incoming results (both initial and follow-up)
  React.useEffect(() => {
    // Ensure result is new, valid, and different from the previous result processed.
    if (result && result.content && result !== prevResult) {
      console.log("BaseAnalysis: New result received:", result);
      console.log("BaseAnalysis: Result has related_questions:", result.related_questions);
      // If there are no related_questions, log it as a potential issue
      if (!result.related_questions || result.related_questions.length === 0) {
        console.warn("BaseAnalysis: No related_questions found in result!");
      }
      setQaHistory(prevHistory => {
        const newHistory = [...prevHistory];
        // Find the first item in history that is marked as processing.
        // This should correspond to the question/workflow that was just run.
        const processingIndex = newHistory.findIndex(item => item.isProcessing);

        if (processingIndex !== -1) {
          console.log("BaseAnalysis: Processing result for item:", newHistory[processingIndex].questionText);
          newHistory[processingIndex] = {
            ...newHistory[processingIndex],
            answer: result,
            isProcessing: false,
            // isInitial is preserved from when the item was added
          };
          // Update depth based on the number of items that now have answers.
          setCurrentAnalysisDepth(newHistory.filter(item => item.answer).length);
        } else {
          // This block is a fallback. Ideally, an item should always be marked `isProcessing`
          // before its corresponding result arrives.
          if (prevHistory.length === 0) {
            // If history was empty, this result is likely for the very first action triggered.
            const triggerText = initialQuestionFromUrl || (workflows.find(w => w.id === selectedWorkflow))?.name;
            if (triggerText) {
              console.warn("BaseAnalysis: Result received, but no processing item found in history. Assuming it's for initial trigger:", triggerText);
              newHistory.push({
                questionText: triggerText,
                answer: result,
                isProcessing: false,
                isInitial: true,
              });
              setCurrentAnalysisDepth(1);
            } else {
              console.error("BaseAnalysis: Result received, but no processing item and no identifiable trigger. Discarding result.", result);
            }
          } else {
            console.warn("BaseAnalysis: Received a result but no matching qaHistory item was actively processing. Result:", result, "Current qaHistory:", newHistory);
            // Potentially, this result could be a duplicate or stale. For now, we don't add it if there's no clear processing item.
          }
        }
        return newHistory;
      });
    }
  }, [result, prevResult, initialQuestionFromUrl, selectedWorkflow, workflows]); // Dependencies help in fallback logic

  const handleAskFollowUpQuestion = (questionText) => {
    // Ensure we don't exceed 5 levels of "why" if an initial analysis has been completed.
    const initialAnalysisCompleted = qaHistory.some(item => item.isInitial && item.answer);
    if (initialAnalysisCompleted && currentAnalysisDepth >= 5) {
      alert('You have completed the 5-Why analysis cycle for this path.');
      return;
    }
    if (!questionText || questionText.trim() === "") {
        alert('Please enter a question.');
        return;
    }

    console.log(`BaseAnalysis: Asking follow-up question: "${questionText}" in context of workflow: ${selectedWorkflow || 'None'}`);
    setQaHistory(prevHistory => [
      ...prevHistory,
      {
        questionText: questionText,
        answer: null,
        isProcessing: true, // Mark as processing
        isInitial: false,   // Follow-up questions are not initial steps
      }
    ]);
    if (onAskQuestion) {
      // Pass the question and the current selectedWorkflow (if any) as context.
      onAskQuestion(questionText, selectedWorkflow || null);
    }
  };

  const handleSelectHistoryItem = (index) => {
    console.log(`BaseAnalysis: Attempting to navigate to history item index: ${index}`);
    // Allow branching if the selected item is not the last one.
    if (index < qaHistory.length - 1) {
      const confirmContinue = window.confirm(
        "You are about to revisit a previous step. Continuing from here will discard subsequent questions and answers in the current path. Do you want to proceed?"
      );
      if (confirmContinue) {
        setQaHistory(prevHistory => prevHistory.slice(0, index + 1));
        setCurrentAnalysisDepth(index + 1); // Depth is now the number of items remaining
        // The UI will then show the RelatedQuestionsPanel based on the new last item.
        // No need to re-fetch, just changing the view.
        console.log("BaseAnalysis: Branched history. New qaHistory:", qaHistory.slice(0, index + 1));
      }
    } else {
      console.log("BaseAnalysis: Selected history item is the last item or analysis is complete. No action taken for branching.");
    }
  };
  
  // Determine if any question is currently being processed for loading indicators
  const isAnyQuestionProcessing = qaHistory.some(item => item.isProcessing);

  // Overall loading state for the analysis section:
  // True if parent says it's loading (e.g. resource details) AND we have no history yet,
  // OR if any question/workflow step within BaseAnalysis is actively processing.
  const overallAnalysisLoading = (loading && qaHistory.length === 0) || isAnyQuestionProcessing;

  if (error) {
    return (
      <Card className="mb-4">
        <CardBody>
          <Alert color="danger">
            <h4>Analysis Error</h4>
            <p>{error.message || (typeof error === 'string' ? error : 'An unexpected error occurred.')}</p>
          </Alert>
        </CardBody>
      </Card>
    );
  }

  // Initial loading spinner for the entire component before any Q&A item is shown
  if (loading && qaHistory.length === 0 && !selectedWorkflow && !initialQuestionFromUrl) {
    return (
      <Card className="mb-4">
        <CardBody className="text-center">
          <Spinner color="primary" style={{ width: '3rem', height: '3rem' }} />
          <p className="mt-2">Loading analysis options...</p>
        </CardBody>
      </Card>
    );
  }
  
  const currentWorkflow = workflows.find(w => w.id === selectedWorkflow);
  const showStartPrompt = !selectedWorkflow && !initialQuestionFromUrl && qaHistory.length === 0 && !overallAnalysisLoading;
  const analysisComplete = qaHistory.some(item => item.isInitial && item.answer) && currentAnalysisDepth >= 5;
  const lastQaItem = qaHistory.length > 0 ? qaHistory[qaHistory.length - 1] : null;
  const showRelatedQuestions = lastQaItem && lastQaItem.answer && !lastQaItem.isProcessing && currentAnalysisDepth < 5;

  return (
    <div className="base-analysis-container">
      {/* Workflow Selection Buttons */}
      {!initialQuestionFromUrl && ( // Only show workflow buttons if not started by a URL question
        <Card className="mb-3">
          <CardBody>
            <h5>Analysis Workflows</h5>
            {workflows && workflows.length > 0 ? (
              <div className="d-flex flex-wrap">
                {workflows.map((wf) => (
                  <Button
                    key={wf.id}
                    color={selectedWorkflow === wf.id ? "primary" : "outline-secondary"}
                    onClick={() => {
                      if (selectedWorkflow !== wf.id) {
                        // Parent's onRunAnalysis will be called by useEffect when selectedWorkflow changes
                        // and qaHistory is reset.
                        // Here, we just update the selectedWorkflow state in the parent.
                        // This requires ResourceAnalysis to have a setSelectedWorkflow prop or similar.
                        // For now, assuming onRunAnalysis in parent also sets selectedWorkflow.
                        // This component itself doesn't set selectedWorkflow directly.
                        // The parent ResourceAnalysis.js calls runAnalysis(workflowId) which sets selectedWorkflow.
                        // So, we should call onRunAnalysis here if we want to change workflow.
                        // However, the current design is that BaseAnalysis receives selectedWorkflow.
                        // The buttons here should ideally call a method passed from parent to change selectedWorkflow.
                        // Let's assume clicking a workflow button calls onRunAnalysis, which implies selection.
                        if (onRunAnalysis) onRunAnalysis(wf.id);
                      }
                    }}
                    className="m-1"
                    disabled={overallAnalysisLoading && selectedWorkflow !== wf.id}
                    id={`workflow-tooltip-${wf.id}`}
                  >
                    {selectedWorkflow === wf.id && isAnyQuestionProcessing && <FaSpinner className="fa-spin me-2" />}
                    {wf.name}
                  </Button>
                ))}
              </div>
            ) : (
              <p>{loading && workflows.length === 0 ? "Loading workflows..." : "No analysis workflows available for this resource type."}</p>
            )}
          </CardBody>
        </Card>
      )}

      {/* Initial Prompt or Loading for Q&A Area */}
      {showStartPrompt && (
        <Card className="mb-3 text-center">
          <CardBody>
            <FaQuestionCircle size="3em" className="text-muted mb-3" />
            <h4>Start Your Analysis</h4>
            <p>Select a workflow above or provide an initial question via URL to begin.</p>
          </CardBody>
        </Card>
      )}
      
      {/* Display Q&A History, Progress, and Related Questions if history exists */}
      {qaHistory.length > 0 && (
        <>
          <QuestionHistory history={qaHistory} onSelectHistoryItem={handleSelectHistoryItem} />
          
          {currentWorkflow && currentWorkflow.name && currentWorkflow.name.toLowerCase().includes("5 why") && ( // Only show for 5-Why type workflows
            <FiveWhyProgress currentStep={currentAnalysisDepth} totalSteps={5} history={qaHistory} />
          )}

          {qaHistory.map((item, index) => (
            <Card key={index} className={`mb-3 ${item.isProcessing ? 'border-primary' : ''}`}>
              <CardBody>
                <div className="d-flex justify-content-between align-items-start">
                  <h5 className="card-title mb-2">
                    {item.isInitial && currentWorkflow ? currentWorkflow.name : item.questionText}
                  </h5>
                  {item.isInitial && <Badge color="success" pill className="ms-2">Initial Analysis</Badge>}
                </div>

                {item.isProcessing ? (
                  <div className="text-center my-3">
                    <Spinner color="primary" />
                    <p className="mt-2">Processing answer...</p>
                  </div>
                ) : item.answer && item.answer.content ? (
                  <div dangerouslySetInnerHTML={{ __html: item.answer.content }} />
                ) : item.answer && typeof item.answer === 'string' ? ( // Basic string answer
                  <p>{item.answer}</p>
                ) : !item.isProcessing && !item.answer ? (
                    <p className="text-muted"><em>No answer received or answer is empty.</em></p>
                ) : null}
              </CardBody>
            </Card>
          ))}

          {showRelatedQuestions && lastQaItem && lastQaItem.answer && lastQaItem.answer.related_questions && (
            <>
              {console.log("Rendering RelatedQuestionsPanel with questions:", lastQaItem.answer.related_questions)}
              <RelatedQuestionsPanel
                relatedQuestions={lastQaItem.answer.related_questions}
                onAskQuestion={handleAskFollowUpQuestion}
                isLoading={isAnyQuestionProcessing} // Pass overall processing state
                currentDepth={currentAnalysisDepth}
                askingQuestionText={qaHistory.find(item => item.isProcessing)?.questionText || null}
              />
            </>
          )}

          {analysisComplete && (
            <FiveWhySummary history={qaHistory} />
          )}
        </>
      )}

      {/* Fallback if loading but no specific prompt/content shown yet */}
      {overallAnalysisLoading && qaHistory.length === 0 && !showStartPrompt && (
         <Card className="mb-3 text-center">
            <CardBody>
                <Spinner color="primary" />
                <p className="mt-2">Loading analysis...</p>
            </CardBody>
        </Card>
      )}
    </div>
  );
};

export default BaseAnalysis;
