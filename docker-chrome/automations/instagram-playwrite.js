// State management
const state = {
  webInfo: null,
  profileData: null,
  timelineEdges: [],
  pageInfo: null,
  totalFetched: 0,
  isProfileComplete: false,
  isTimelineComplete: false,
  isComplete: false,
  lastRequestInfo: null
};

// Helper: Fetch web_info to get logged-in user data
const fetchWebInfo = async () => {
  const result = await playwright.evaluate(`
    (async () => {
      try {
        const response = await fetch("https://www.instagram.com/accounts/web_info/", {
          headers: { "X-Requested-With": "XMLHttpRequest" }
        });
        if (!response.ok) return null;

        const html = await response.text();
        const parser = new DOMParser();
        const doc = parser.parseFromString(html, "text/html");
        const scripts = doc.querySelectorAll('script[type="application/json"][data-sjs]');

        // Recursive helper to find the 'PolarisViewer' array definition
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

        // Iterate through scripts to find the user data
        let foundData = null;
        for (const script of scripts) {
          try {
            const jsonContent = JSON.parse(script.textContent);
            foundData = findPolarisData(jsonContent);
            if (foundData) break;
          } catch (e) {}
        }

        if (foundData && foundData.data) {
          return foundData.data;
        }
        return null;
      } catch (err) {
        return null;
      }
    })()
  `);
  return result;
};

// Navigate to Instagram
await playwright.goto('https://www.instagram.com');
playwright.setData('status', 'Navigated to Instagram');
await playwright.sleep(2000);

// Check login status via web_info
playwright.setData('status', 'Checking login status...');
const webInfo = await fetchWebInfo();
state.webInfo = webInfo;
playwright.setData('webInfo', webInfo);

const isLoggedIn = webInfo && webInfo.username;
playwright.setData('isLoggedIn', !!isLoggedIn);

if (!isLoggedIn) {
  // Navigate to login page if needed
  await playwright.goto('https://www.instagram.com/accounts/login/');
  await playwright.sleep(1000);

  // Wait for user to log in - check periodically for login completion
  await playwright.promptUser(
    'Please log in to Instagram.',
    async () => {
      const info = await fetchWebInfo();
      return !!(info && info.username);
    },
    2000 // Poll every 2 seconds
  );

  // Re-fetch web info after login
  const newWebInfo = await fetchWebInfo();
  state.webInfo = newWebInfo;
  playwright.setData('webInfo', newWebInfo);
  playwright.setData('loginCompleted', true);
}

// Now we're logged in - get the username
const username = state.webInfo?.username;
if (!username) {
  playwright.setData('error', 'Could not determine username from web_info');
  return { error: 'Could not determine username' };
}

playwright.setData('username', username);
playwright.setData('status', `Logged in as @${username}`);

// Set up network captures BEFORE navigating to profile
// Capture profile data
playwright.captureNetwork({
  urlPattern: '/graphql',
  bodyPattern: 'PolarisProfilePageContentQuery|ProfilePageQuery|UserByUsernameQuery',
  key: 'profileResponse'
});

// Capture posts/timeline data
playwright.captureNetwork({
  urlPattern: '/graphql',
  bodyPattern: 'PolarisProfilePostsQuery|PolarisProfilePostsTabContentQuery_connection|ProfilePostsQuery|UserMediaQuery',
  key: 'postsResponse'
});

playwright.setData('status', 'Network capture configured');

// Navigate to user's profile
playwright.setData('status', `Navigating to profile: @${username}`);
await playwright.goto(`https://www.instagram.com/${username}/`);
await playwright.sleep(3000);

// Wait for profile data
playwright.setData('status', 'Waiting for profile data...');
let profileData = null;
let postsData = null;
let attempts = 0;
const maxAttempts = 30;

while (attempts < maxAttempts && (!profileData || !postsData)) {
  await playwright.sleep(1000);
  attempts++;
  playwright.setData('captureAttempts', attempts);

  if (!profileData) {
    profileData = playwright.getCapturedResponse('profileResponse');
    if (profileData) {
      playwright.setData('profileCaptured', true);
      playwright.setData('status', 'Profile data captured!');

      // Extract profile data
      const userData = profileData?.data?.user;
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
          viewer_data: profileData.data?.viewer,
          collected_at: new Date().toISOString()
        };
        state.isProfileComplete = true;
        playwright.setData('profile', state.profileData);
      }
    }
  }

  if (!postsData) {
    postsData = playwright.getCapturedResponse('postsResponse');
    if (postsData) {
      playwright.setData('postsCaptured', true);
    }
  }
}

// If we didn't get posts data, try scrolling to trigger loading
if (!postsData) {
  playwright.setData('status', 'Scrolling to load posts...');
  await playwright.evaluate(`window.scrollTo(0, document.body.scrollHeight)`);
  await playwright.sleep(2000);
  postsData = playwright.getCapturedResponse('postsResponse');
}

// Process initial posts data
if (postsData) {
  const timelineData = postsData?.data?.xdt_api__v1__feed__user_timeline_graphql_connection;
  if (timelineData) {
    const { edges, page_info } = timelineData;
    if (edges && Array.isArray(edges)) {
      state.timelineEdges = edges;
      state.pageInfo = page_info;
      state.totalFetched = edges.length;
      playwright.setData('postsCount', state.totalFetched);
      playwright.setData('status', `Captured ${state.totalFetched} posts`);

      // Store last request info for pagination
      // We'll need to fetch more pages if has_next_page is true
      if (page_info?.has_next_page && page_info?.end_cursor) {
        playwright.setData('status', `Fetching more posts... (${state.totalFetched} so far)`);

        // Auto-fetch remaining pages by scrolling and waiting for network
        let hasMore = true;
        let scrollAttempts = 0;
        const maxScrollAttempts = 20;

        while (hasMore && scrollAttempts < maxScrollAttempts) {
          scrollAttempts++;

          // Clear the capture to get fresh data
          playwright.clearNetworkCaptures();
          playwright.captureNetwork({
            urlPattern: '/graphql',
            bodyPattern: 'PolarisProfilePostsQuery|PolarisProfilePostsTabContentQuery_connection|ProfilePostsQuery|UserMediaQuery',
            key: 'postsResponse'
          });

          // Scroll down
          await playwright.evaluate(`window.scrollTo(0, document.body.scrollHeight)`);
          await playwright.sleep(2000);

          // Check for new posts data
          const nextPostsData = playwright.getCapturedResponse('postsResponse');
          if (nextPostsData) {
            const nextTimelineData = nextPostsData?.data?.xdt_api__v1__feed__user_timeline_graphql_connection;
            if (nextTimelineData?.edges) {
              const { edges: newEdges, page_info: newPageInfo } = nextTimelineData;

              // Deduplicate edges
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
                playwright.setData('postsCount', state.totalFetched);
                playwright.setData('status', `Captured ${state.totalFetched} posts (${uniqueNewEdges.length} new)`);
              }

              hasMore = newPageInfo?.has_next_page && newPageInfo?.end_cursor && uniqueNewEdges.length > 0;
            } else {
              hasMore = false;
            }
          } else {
            // No new data captured - might have finished loading
            hasMore = false;
          }
        }
      }

      state.isTimelineComplete = true;
    }
  }
}

// Transform data to schema format (matching instagram-inpage.js)
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

  // Include all webInfo properties merged with profile data
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
    version: "2.0.0-playwright",
    platform: "instagram",
    // Include webInfo data
    ...(state.webInfo || {})
  };
};

// Build final result
state.isComplete = state.isProfileComplete;
playwright.setData('status', 'Data collection complete!');

const result = transformDataForSchema();
if (result) {
  playwright.setData('result', result);
  playwright.setData('postsCount', result.posts?.length || 0);
  playwright.setData('status', `Complete! ${result.posts?.length || 0} posts collected for @${result.username}`);

  // Store individual keys for easy viewing in data panel
  playwright.setData('username', result.username);
  playwright.setData('bio', result.bio);
  playwright.setData('follower_count', result.follower_count);
  playwright.setData('following_count', result.following_count);
  playwright.setData('posts', result.posts);
  playwright.setData('complete', true);

  return result;
} else {
  playwright.setData('error', 'Failed to transform data');
  playwright.setData('raw_profile', state.profileData);
  playwright.setData('raw_edges', state.timelineEdges);
  return {
    error: 'Failed to transform data',
    profile: state.profileData,
    timeline: state.timelineEdges
  };
}
