/**
 * 测试 Google Gemini API 的模型列表和实际调用
 * 验证 /models 返回的格式 vs chat/completions 需要的格式
 */

import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

function loadEnv() {
    const content = fs.readFileSync(path.join(__dirname, '..', '.env'), 'utf-8');
    const env = {};
    for (const line of content.split('\n')) {
        const trimmed = line.trim();
        if (!trimmed || trimmed.startsWith('#')) continue;
        const [key, ...valueParts] = trimmed.split('=');
        if (key && valueParts.length > 0) {
            env[key.trim()] = valueParts.join('=').trim();
        }
    }
    return env;
}

const env = loadEnv();
const API_KEY = env.GEMENI_KEY || env.GEMINI_KEY;
const BASE_URL = env.URL || 'https://generativelanguage.googleapis.com/v1beta/openai';

if (!API_KEY) {
    console.error('❌ GEMENI_KEY not found');
    process.exit(1);
}

async function fetchModels() {
    console.log('=== 1. 获取模型列表 ===');
    console.log(`URL: ${BASE_URL}/models`);
    
    try {
        const response = await fetch(`${BASE_URL}/models`, {
            headers: {
                'Authorization': `Bearer ${API_KEY}`
            }
        });
        
        if (!response.ok) {
            console.error(`❌ Failed: HTTP ${response.status}`);
            const text = await response.text();
            console.error('Response:', text.substring(0, 500));
            return [];
        }
        
        const data = await response.json();
        const models = data.data || [];
        
        console.log(`✅ 找到 ${models.length} 个模型`);
        console.log('\n前10个模型:');
        models.slice(0, 10).forEach((m, i) => {
            console.log(`  ${i + 1}. ${m.id}`);
        });
        
        return models.map(m => m.id);
    } catch (error) {
        console.error('❌ Error:', error.message);
        return [];
    }
}

async function testChatCompletion(modelName) {
    console.log(`\n=== 2. 测试 chat/completions ===`);
    console.log(`Model: ${modelName}`);
    
    try {
        const response = await fetch(`${BASE_URL}/chat/completions`, {
            method: 'POST',
            headers: {
                'Authorization': `Bearer ${API_KEY}`,
                'Content-Type': 'application/json'
            },
            body: JSON.stringify({
                model: modelName,
                messages: [{ role: 'user', content: 'Hi' }],
                max_tokens: 10
            })
        });
        
        const data = await response.json();
        
        if (response.ok) {
            console.log(`✅ Success! Response: "${data.choices?.[0]?.message?.content?.trim()}"`);
            return true;
        } else {
            console.log(`❌ Failed: HTTP ${response.status}`);
            console.log('Error:', JSON.stringify(data, null, 2).substring(0, 500));
            return false;
        }
    } catch (error) {
        console.error('❌ Error:', error.message);
        return false;
    }
}

async function main() {
    console.log('========================================');
    console.log('Google Gemini API 模型格式验证');
    console.log('========================================\n');
    
    // 1. 获取模型列表
    const models = await fetchModels();
    
    if (models.length === 0) {
        console.log('\n无法获取模型列表，直接测试常见格式...');
    } else {
        // 2. 尝试第一个带 models/ 前缀的模型
        const withPrefix = models.find(m => m.startsWith('models/'));
        if (withPrefix) {
            console.log(`\n--- 测试带前缀格式: ${withPrefix} ---`);
            const withPrefixResult = await testChatCompletion(withPrefix);
            
            // 3. 尝试去掉前缀
            const withoutPrefix = withPrefix.replace('models/', '');
            console.log(`\n--- 测试无前缀格式: ${withoutPrefix} ---`);
            const withoutPrefixResult = await testChatCompletion(withoutPrefix);
            
            // 总结
            console.log('\n========================================');
            console.log('验证结果');
            console.log('========================================');
            console.log(`带 models/ 前缀: ${withPrefixResult ? '✅ 可用' : '❌ 不可用'}`);
            console.log(`无 models/ 前缀: ${withoutPrefixResult ? '✅ 可用' : '❌ 不可用'}`);
            
            if (withPrefixResult && !withoutPrefixResult) {
                console.log('\n⚠️ 结论: 必须使用带 models/ 前缀的格式');
            } else if (!withPrefixResult && withoutPrefixResult) {
                console.log('\n⚠️ 结论: 必须去掉 models/ 前缀');
            } else if (withPrefixResult && withoutPrefixResult) {
                console.log('\n✅ 两种格式都可用');
            } else {
                console.log('\n❌ 两种格式都不可用，可能是其他问题');
            }
        }
    }
    
    // 4. 测试用户配置中的模型
    const userModel = env.GEMENI_MODEL || env.GEMINI_MODEL;
    if (userModel) {
        console.log(`\n========================================`);
        console.log(`测试用户配置的模型: ${userModel}`);
        console.log('========================================');
        await testChatCompletion(userModel);
    }
}

main().catch(console.error);
