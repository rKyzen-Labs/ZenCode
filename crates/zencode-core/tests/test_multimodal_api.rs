use zencode_core::{CredentialService, GeminiPlannerClient, MultimodalAttachment, PlanningContext};

#[tokio::test]
async fn test_real_gemini_multimodal_call() {
    let api_key = match CredentialService::get_gemini_api_key() {
        Ok(k) if !k.is_empty() => k,
        _ => {
            println!("Skipping: No Gemini API key present in Windows Credential Manager");
            return;
        }
    };

    println!("Testing Gemini API with real key...");

    // Create a valid PDF with actual text content stream: "ZENCODE DESIGN SPEC: Color is Crimson, Button says Launch Rocket"
    let pdf_content = b"%PDF-1.4\n\
1 0 obj\n<</Type/Catalog/Pages 2 0 R>>\nendobj\n\
2 0 obj\n<</Type/Pages/Count 1/Kids[3 0 R]>>\nendobj\n\
3 0 obj\n<</Type/Page/MediaBox[0 0 612 792]/Parent 2 0 R/Resources<</Font<</F1 4 0 R>>>>/Contents 5 0 R>>\nendobj\n\
4 0 obj\n<</Type/Font/Subtype/Type1/BaseFont/Helvetica>>\nendobj\n\
5 0 obj\n<</Length 115>>\nstream\n\
BT\n\
/F1 24 Tf\n\
50 700 Td\n\
(ZENCODE DESIGN SPEC: Color is Crimson, Button says Launch Rocket) Tj\n\
ET\n\
endstream\nendobj\n\
xref\n\
0 6\n\
0000000000 65535 f \n\
0000000009 00000 n \n\
0000000058 00000 n \n\
0000000115 00000 n \n\
0000000228 00000 n \n\
0000000295 00000 n \n\
trailer\n<</Size 6/Root 1 0 R>>\n\
startxref\n\
462\n\
%%EOF";

    let att = MultimodalAttachment::from_bytes("design_spec.pdf", "application/pdf", pdf_content);

    let context = PlanningContext {
        task_description: "Develop the website specified in design_spec.pdf".into(),
        project_type: "Web / Static".into(),
        shallow_tree: vec!["design_spec.pdf".into()],
        multimodal_attachments: vec![att],
        ..Default::default()
    };

    let client = GeminiPlannerClient::new();
    match client.plan(&api_key, &context).await {
        Ok((plan, usage)) => {
            println!("SUCCESS! Plan generated with multimodal PDF:");
            println!("Summary: {}", plan.task_summary);
            println!("Is Actionable: {}", plan.is_actionable);
            println!("Conversational reply: {:?}", plan.conversational_reply);
            println!("Tokens: {}", usage.total_token_count);
        }
        Err(e) => {
            println!("Gemini Plan response: {:?}", e);
            // If Google's free tier endpoint is temporarily unavailable (503/429), log but don't fail CI
            let err_str = format!("{:?}", e);
            if err_str.contains("503") || err_str.contains("429") || err_str.contains("UNAVAILABLE") {
                println!("Live API temporary high demand (503/429): Skipping test failure.");
            } else {
                panic!("Gemini Plan failed: {:?}", e);
            }
        }
    }
}
