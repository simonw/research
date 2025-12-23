// State management
const state = {
  webInfo: null,
  profileData: null,
  timelineEdges: [],
  pageInfo: null,
  totalFetched: 0,
  isProfileComplete: false,
  isTimelineComplete: false,
  isComplete: false
};

// Helper: Fetch web_info to get logged-in user data
const fetchWebInfo = async () => {
  try {
    const result = await page.evaluate(`
      (async () => {
        try {
          const response = await fetch("https://www.instagram.com/accounts/web_info/", {
            headers: { "X-Requested-With": "XMLHttpRequest" }
          });
          if (!response.ok) return { error: 'response not ok', status: response.status };

          const html = await response.text();
          const parser = new DOMParser();
          const doc = parser.parseFromString(html, "text/html");
          const scripts = doc.querySelectorAll('script[type="application/json"][data-sjs]');

          const findPolarisData = (obj) => {
            if (!obj || typeof obj !== 'object') return null;
            if (Array.isArray(obj) && obj[0] === 'PolarisViewer' && obj.length >= 3) {
              return obj[2];
            }
            for (const key in obj) {
              if (Object.prototype.hasOwnProperty.call(obj, key)) {
                const found = findPolarisData(obj[key]);
                if (found) return found;
              }
            }
            return null;
          };

          let foundData = null;
          for (const script of scripts) {
            try {
              const jsonContent = JSON.parse(script.textContent);
              foundData = findPolarisData(jsonContent);
              if (foundData) break;
            } catch (e) {}
          }

          if (foundData && foundData.data) {
            return { success: true, data: foundData.data };
          }
          return { error: 'no polaris data found', scriptsCount: scripts.length };
        } catch (err) {
          return { error: err.message };
        }
      })()
    `);
    if (result?.success) {
      return result.data;
    }
    return null;
  } catch (err) {
    return null;
  }
};

// Navigate to Instagram
await page.goto('https://www.instagram.com');
await page.setData('status', 'Navigated to Instagram');
await page.sleep(2000);

// Check login status via web_info
await page.setData('status', 'Checking login status...');
const webInfo = await fetchWebInfo();
state.webInfo = webInfo;

const isLoggedIn = webInfo && webInfo.username;

if (!isLoggedIn) {
  await page.goto('https://www.instagram.com/accounts/login/');
  await page.sleep(1000);

  // Wait for user to log in - callback auto-detects login completion
  await page.promptUser(
    'Please log in to Instagram.',
    async () => {
      const info = await fetchWebInfo();
      return !!(info && info.username);
    },
    2000
  );

  // Re-fetch web info after login
  const newWebInfo = await fetchWebInfo();
  state.webInfo = newWebInfo;
  await page.setData('status', 'Login completed');
}

// Get the username
const username = state.webInfo?.username;
if (!username) {
  await page.setData('error', 'Could not determine username');
  return { error: 'Could not determine username' };
}

await page.setData('status', `Logged in as @${username}`);

// Set up network captures BEFORE navigating to profile
await page.captureNetwork({
  urlPattern: '/graphql',
  bodyPattern: 'PolarisProfilePageContentQuery|ProfilePageQuery|UserByUsernameQuery',
  key: 'profileResponse'
});

await page.captureNetwork({
  urlPattern: '/graphql',
  bodyPattern: 'PolarisProfilePostsQuery|PolarisProfilePostsTabContentQuery_connection|ProfilePostsQuery|UserMediaQuery',
  key: 'postsResponse'
});

await page.setData('status', 'Network capture configured');

// Navigate to user's profile
await page.setData('status', `Navigating to profile: @${username}`);
await page.goto(`https://www.instagram.com/${username}/`);
await page.sleep(3000);

// Wait for profile data
await page.setData('status', 'Waiting for profile data...');
let profileData = null;
let postsData = null;
let attempts = 0;
const maxAttempts = 30;

while (attempts < maxAttempts && (!profileData || !postsData)) {
  await page.sleep(1000);
  attempts++;

  if (!profileData) {
    profileData = await page.getCapturedResponse('profileResponse');
    if (profileData) {
      await page.setData('status', 'Profile data captured!');

      const userData = profileData?.data?.data?.user;
      if (userData) {
        state.profileData = {
          username: userData.username,
          full_name: userData.full_name,
          pk: userData.pk,
          id: userData.id,
          biography: userData.biography,
          follower_count: userData.follower_count,
          following_count: userData.following_count,
          media_count: userData.media_count,
          total_clips_count: userData.total_clips_count,
          profile_pic_url: userData.profile_pic_url,
          hd_profile_pic_url: userData.hd_profile_pic_url_info?.url,
          has_profile_pic: userData.has_profile_pic,
          is_private: userData.is_private,
          is_verified: userData.is_verified,
          is_business: userData.is_business,
          is_professional_account: userData.is_professional_account,
          account_type: userData.account_type,
          external_url: userData.external_url,
          external_lynx_url: userData.external_lynx_url,
          bio_links: userData.bio_links,
          linked_fb_info: userData.linked_fb_info,
          pronouns: userData.pronouns,
          account_badges: userData.account_badges,
          has_story_archive: userData.has_story_archive,
          viewer_data: profileData.data?.data?.viewer,
          collected_at: new Date().toISOString()
        };
        state.isProfileComplete = true;
        await page.setData('profile', state.profileData);
      }
    }
  }

  if (!postsData) {
    postsData = await page.getCapturedResponse('postsResponse');
    if (postsData) {
      await page.setData('status', 'Posts data captured!');
    }
  }
}

// If we didn't get posts data, try scrolling to trigger loading
if (!postsData) {
  await page.setData('status', 'Scrolling to load posts...');
  await page.evaluate(`window.scrollTo(0, document.body.scrollHeight)`);
  await page.sleep(2000);
  postsData = await page.getCapturedResponse('postsResponse');
}

// Process initial posts data
if (postsData) {
  const timelineData = postsData?.data?.data?.xdt_api__v1__feed__user_timeline_graphql_connection;
  if (timelineData) {
    const { edges, page_info } = timelineData;
    if (edges && Array.isArray(edges)) {
      state.timelineEdges = edges;
      state.pageInfo = page_info;
      state.totalFetched = edges.length;
      await page.setData('status', `Captured ${state.totalFetched} posts`);

      // Fetch more pages if available
      if (page_info?.has_next_page && page_info?.end_cursor) {
        await page.setData('status', `Fetching more posts... (${state.totalFetched} so far)`);

        let hasMore = true;
        let scrollAttempts = 0;
        const maxScrollAttempts = 20;

        while (hasMore && scrollAttempts < maxScrollAttempts) {
          scrollAttempts++;

          await page.clearNetworkCaptures();
          await page.captureNetwork({
            urlPattern: '/graphql',
            bodyPattern: 'PolarisProfilePostsQuery|PolarisProfilePostsTabContentQuery_connection|ProfilePostsQuery|UserMediaQuery',
            key: 'postsResponse'
          });

          await page.evaluate(`window.scrollTo(0, document.body.scrollHeight)`);
          await page.sleep(2000);

          const nextPostsData = await page.getCapturedResponse('postsResponse');
          if (nextPostsData) {
            const nextTimelineData = nextPostsData?.data?.data?.xdt_api__v1__feed__user_timeline_graphql_connection;
            if (nextTimelineData?.edges) {
              const { edges: newEdges, page_info: newPageInfo } = nextTimelineData;

              const existingIds = new Set(
                state.timelineEdges.map(edge =>
                  edge.node?.id || edge.node?.pk || edge.node?.media_id || edge.node?.code
                ).filter(Boolean)
              );

              const uniqueNewEdges = newEdges.filter(edge => {
                const nodeId = edge.node?.id || edge.node?.pk || edge.node?.media_id || edge.node?.code;
                return nodeId && !existingIds.has(nodeId);
              });

              if (uniqueNewEdges.length > 0) {
                state.timelineEdges = [...state.timelineEdges, ...uniqueNewEdges];
                state.pageInfo = newPageInfo;
                state.totalFetched = state.timelineEdges.length;
                await page.setData('status', `Captured ${state.totalFetched} posts`);
              }

              hasMore = newPageInfo?.has_next_page && newPageInfo?.end_cursor && uniqueNewEdges.length > 0;
            } else {
              hasMore = false;
            }
          } else {
            hasMore = false;
          }
        }
      }

      state.isTimelineComplete = true;
    }
  }
}

// Transform data to schema format
const transformDataForSchema = () => {
  const profile = state.profileData;
  const edges = state.timelineEdges;

  if (!profile) {
    return null;
  }

  const posts = (edges || []).map((edge) => {
    const node = edge.node;
    const imgUrl = node.image_versions2?.candidates?.[0]?.url ||
      node.carousel_media?.[0]?.image_versions2?.candidates?.[0]?.url || "";
    const caption = node.caption?.text || "";
    const numOfLikes = node.like_count || 0;
    const whoLiked = (node.facepile_top_likers || []).map((liker) => ({
      profile_pic_url: liker.profile_pic_url || "",
      pk: liker.pk || liker.id || "",
      username: liker.username || "",
      id: liker.id || liker.pk || ""
    }));

    return {
      img_url: imgUrl,
      caption: caption,
      num_of_likes: numOfLikes,
      who_liked: whoLiked
    };
  });

  return {
    username: profile.username,
    bio: profile.biography,
    full_name: profile.full_name,
    follower_count: profile.follower_count,
    following_count: profile.following_count,
    media_count: profile.media_count,
    profile_pic_url: profile.profile_pic_url,
    is_private: profile.is_private,
    is_verified: profile.is_verified,
    is_business: profile.is_business,
    external_url: profile.external_url,
    posts: posts,
    timestamp: new Date().toISOString(),
    version: "2.0.0-page",
    platform: "instagram",
    ...(state.webInfo || {})
  };
};

// Build final result
state.isComplete = state.isProfileComplete;
const result = transformDataForSchema();

if (result) {
  await page.setData('result', result);
  await page.setData('status', `Complete! ${result.posts?.length || 0} posts collected for @${result.username}`);
  return { success: true, data: result };
} else {
  await page.setData('error', 'Failed to transform data');
  return { success: false, error: 'Failed to transform data' };
}
